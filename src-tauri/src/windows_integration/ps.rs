//! System information and settings through PowerShell and Windows tools
//! (`powercfg`, `netsh`, WMI, WinRT, UI Automation). Scripts are fixed in
//! code; user values are passed as escaped literals, never as code.

use std::time::Duration;

use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{AppError, AppResult};

/// Single-quoted PowerShell string literal.
pub fn ps_str(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// Runs a script in a hidden PowerShell (UTF-8 output, -EncodedCommand).
pub async fn run(script: &str, timeout: Duration) -> AppResult<String> {
    let full = format!("$ProgressPreference='SilentlyContinue';[Console]::OutputEncoding=[Text.Encoding]::UTF8;{script}");
    let utf16: Vec<u8> = full.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let encoded = base64::engine::general_purpose::STANDARD.encode(utf16);
    let mut cmd = tokio::process::Command::new("powershell.exe");
    cmd.args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-EncodedCommand", &encoded]);
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000);
    cmd.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).kill_on_drop(true);
    let child = cmd.spawn().map_err(|e| AppError::Other(anyhow::anyhow!("PowerShell недоступен: {e}")))?;
    let out = tokio::time::timeout(timeout, child.wait_with_output())
        .await
        .map_err(|_| AppError::Other(anyhow::anyhow!("PowerShell не ответил за {} с", timeout.as_secs())))?
        .map_err(anyhow::Error::from)?;
    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let first = err.lines().find(|l| !l.trim().is_empty()).unwrap_or("ошибка PowerShell").trim().to_string();
        return Err(AppError::Other(anyhow::anyhow!(first)));
    }
    Ok(stdout)
}

/// PowerShell's ConvertTo-Json gives an object for one item and an array for many.
fn as_list(v: Value) -> Vec<Value> {
    match v {
        Value::Array(a) => a,
        Value::Null => vec![],
        other => vec![other],
    }
}

async fn json(script: &str, secs: u64) -> AppResult<Value> {
    let out = run(script, Duration::from_secs(secs)).await?;
    if out.is_empty() {
        return Ok(Value::Null);
    }
    serde_json::from_str(&out).map_err(|e| AppError::Other(anyhow::anyhow!("неожиданный ответ PowerShell: {e}")))
}

// --- installed apps ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledApp {
    pub name: String,
    /// AppUserModelID or path; launched as `shell:AppsFolder\<id>`.
    pub app_id: String,
}

/// Start menu apps, classic and Store (`Get-StartApps`).
pub async fn start_apps() -> AppResult<Vec<InstalledApp>> {
    let v = json("Get-StartApps | Select-Object Name, AppID | ConvertTo-Json -Compress", 30).await?;
    let mut apps: Vec<InstalledApp> = as_list(v)
        .into_iter()
        .filter_map(|a| Some(InstalledApp { name: a["Name"].as_str()?.to_string(), app_id: a["AppID"].as_str()?.to_string() }))
        .filter(|a| !a.name.is_empty() && !a.app_id.is_empty())
        .collect();
    apps.sort_by_key(|a| a.name.to_lowercase());
    apps.dedup_by(|a, b| a.app_id == b.app_id);
    Ok(apps)
}

// --- brightness (internal displays only) ---

pub async fn brightness() -> AppResult<Option<u8>> {
    let out = run(
        "try { (Get-CimInstance -Namespace root/WMI -ClassName WmiMonitorBrightness -ErrorAction Stop | Select-Object -First 1).CurrentBrightness } catch { '' }",
        Duration::from_secs(15),
    )
    .await?;
    Ok(out.trim().parse().ok())
}

pub async fn set_brightness(level: u8) -> AppResult<()> {
    let script = format!(
        "$m = Get-CimInstance -Namespace root/WMI -ClassName WmiMonitorBrightnessMethods -ErrorAction Stop | Select-Object -First 1; \
         Invoke-CimMethod -InputObject $m -MethodName WmiSetBrightness -Arguments @{{Timeout=1; Brightness=[byte]{}}} | Out-Null",
        level.min(100)
    );
    run(&script, Duration::from_secs(15)).await.map(|_| ()).map_err(|_| AppError::Invalid("яркость этого экрана нельзя менять программно (обычно — только у встроенных экранов ноутбуков)".into()))
}

// --- network and radios ---

#[derive(Debug, Clone, Default, Serialize)]
pub struct WifiStatus {
    pub available: bool,
    pub connected: bool,
    pub adapter: String,
    pub ssid: Option<String>,
    pub signal: Option<u8>,
}

/// Adapter state from Get-NetAdapter (not localized) + SSID/signal from netsh.
pub async fn wifi() -> AppResult<WifiStatus> {
    let v = json(
        "$a = Get-NetAdapter -Physical -ErrorAction SilentlyContinue | Where-Object { $_.NdisPhysicalMedium -eq 9 -or $_.InterfaceDescription -match 'Wi-?Fi|Wireless|802\\.11' } | Select-Object -First 1; \
         if (-not $a) { '{}' } else { \
           $n = (netsh wlan show interfaces) -join \"`n\"; \
           $ssid = if ($n -match '(?m)^\\s*SSID\\s*:\\s*(.+)$') { $Matches[1].Trim() } else { $null }; \
           $sig = if ($n -match '(?m):\\s*(\\d{1,3})\\s*%') { [int]$Matches[1] } else { $null }; \
           @{ adapter = $a.Name; status = [string]$a.Status; ssid = $ssid; signal = $sig } | ConvertTo-Json -Compress }",
        20,
    )
    .await?;
    if v.get("adapter").is_none() {
        return Ok(WifiStatus::default());
    }
    let connected = v["status"].as_str() == Some("Up");
    Ok(WifiStatus {
        available: true,
        connected,
        adapter: v["adapter"].as_str().unwrap_or("").into(),
        ssid: connected.then(|| v["ssid"].as_str().map(String::from)).flatten(),
        signal: connected.then(|| v["signal"].as_u64().map(|s| s.min(100) as u8)).flatten(),
    })
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct BluetoothStatus {
    pub available: bool,
    /// The radio is on (adapter works and the Bluetooth service runs).
    pub enabled: bool,
    pub adapter: String,
    /// Paired devices that are present.
    pub devices: Vec<String>,
}

pub async fn bluetooth() -> AppResult<BluetoothStatus> {
    let v = json(
        "$d = @(Get-PnpDevice -Class Bluetooth -PresentOnly -ErrorAction SilentlyContinue); \
         $ad = $d | Where-Object { $_.FriendlyName -match 'Adapter|Radio|Wireless Bluetooth|Generic Bluetooth|Intel|Realtek|Qualcomm|MediaTek|Broadcom' } | Select-Object -First 1; \
         $svc = (Get-Service bthserv -ErrorAction SilentlyContinue).Status; \
         $dev = @($d | Where-Object { $_.Status -eq 'OK' -and $_.FriendlyName -notmatch 'Enumerator|Adapter|Radio|Generic|Protocol|Service|RFCOMM|Microsoft Bluetooth|Avrcp|Transport' } | ForEach-Object { $_.FriendlyName } | Sort-Object -Unique); \
         @{ count = $d.Count; adapter = if ($ad) { $ad.FriendlyName } else { '' }; adapter_ok = if ($ad) { $ad.Status -eq 'OK' } else { $false }; service = [string]$svc; devices = $dev } | ConvertTo-Json -Compress",
        20,
    )
    .await?;
    let available = v["count"].as_u64().unwrap_or(0) > 0;
    Ok(BluetoothStatus {
        available,
        enabled: available && v["adapter_ok"].as_bool().unwrap_or(false) && v["service"].as_str() == Some("Running"),
        adapter: v["adapter"].as_str().unwrap_or("").into(),
        devices: as_list(v["devices"].clone()).into_iter().filter_map(|d| d.as_str().map(String::from)).collect(),
    })
}

// --- power plans ---

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PowerPlan {
    pub guid: String,
    pub name: String,
    pub active: bool,
}

/// Parses `powercfg /list` in any language: GUID, (name) and the * marker.
pub fn parse_power_plans(out: &str) -> Vec<PowerPlan> {
    out.lines()
        .filter_map(|l| {
            let guid = l.split_whitespace().find(|w| is_guid(w))?.to_lowercase();
            let name = l.find('(').and_then(|a| l.rfind(')').filter(|b| *b > a).map(|b| l[a + 1..b].trim().to_string())).unwrap_or_else(|| guid.clone());
            Some(PowerPlan { guid, name, active: l.trim_end().ends_with('*') })
        })
        .collect()
}

pub fn is_guid(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    parts.len() == 5 && [8, 4, 4, 4, 12].iter().zip(&parts).all(|(n, p)| p.len() == *n && p.chars().all(|c| c.is_ascii_hexdigit()))
}

pub async fn power_plans() -> AppResult<Vec<PowerPlan>> {
    let out = crate::clipboard_hub::actions::run_command("powercfg /list", Duration::from_secs(15)).await?;
    Ok(parse_power_plans(&out.stdout))
}

pub async fn set_power_plan(guid: &str) -> AppResult<()> {
    if !is_guid(guid) {
        return Err(AppError::Invalid("неверный идентификатор схемы".into()));
    }
    let out = crate::clipboard_hub::actions::run_command(&format!("powercfg /setactive {guid}"), Duration::from_secs(15)).await?;
    if out.code != Some(0) {
        return Err(AppError::Other(anyhow::anyhow!("powercfg: {}", out.stderr.trim())));
    }
    Ok(())
}

// --- virtual desktops ---

#[derive(Debug, Clone, Serialize)]
pub struct Desktops {
    pub count: u32,
    /// 1-based, when Windows reports it.
    pub current: Option<u32>,
}

/// Reads Explorer's registry state (there is no public API for virtual desktops).
pub async fn desktops() -> AppResult<Desktops> {
    let v = json(
        "$k = 'HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\VirtualDesktops'; \
         $ids = (Get-ItemProperty $k -ErrorAction SilentlyContinue).VirtualDesktopIDs; \
         $cur = (Get-ItemProperty $k -ErrorAction SilentlyContinue).CurrentVirtualDesktop; \
         $n = if ($ids) { [int]($ids.Length / 16) } else { 1 }; $i = $null; \
         if ($ids -and $cur) { for ($j = 0; $j -lt $n; $j++) { if ([BitConverter]::ToString($ids[($j*16)..($j*16+15)]) -eq [BitConverter]::ToString($cur)) { $i = $j + 1 } } }; \
         @{ count = $n; current = $i } | ConvertTo-Json -Compress",
        15,
    )
    .await?;
    Ok(Desktops { count: v["count"].as_u64().unwrap_or(1).max(1) as u32, current: v["current"].as_u64().map(|c| c as u32) })
}

// --- notifications and UI Automation ---

/// A Windows toast shown on behalf of PowerShell (no extra app registration).
pub async fn toast(title: &str, body: &str) -> AppResult<()> {
    let script = format!(
        "[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] > $null; \
         $t = [Windows.UI.Notifications.ToastNotificationManager]::GetTemplateContent([Windows.UI.Notifications.ToastTemplateType]::ToastText02); \
         $x = $t.GetElementsByTagName('text'); $x.Item(0).AppendChild($t.CreateTextNode({})) > $null; $x.Item(1).AppendChild($t.CreateTextNode({})) > $null; \
         $n = [Windows.UI.Notifications.ToastNotification]::new($t); \
         [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('{{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}}\\WindowsPowerShell\\v1.0\\powershell.exe').Show($n)",
        ps_str(title),
        ps_str(body)
    );
    run(&script, Duration::from_secs(20)).await.map(|_| ())
}

#[derive(Debug, Clone, Serialize)]
pub struct UiElement {
    pub depth: u32,
    pub name: String,
    pub control_type: String,
    pub automation_id: String,
}

/// Reads (only reads) up to 150 UI elements, two levels deep.
pub async fn uia_inspect(hwnd: i64) -> AppResult<Vec<UiElement>> {
    let script = format!(
        "Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes; \
         $root = [System.Windows.Automation.AutomationElement]::FromHandle([IntPtr]{hwnd}); \
         $w = [System.Windows.Automation.TreeWalker]::ControlViewWalker; $script:n = 0; \
         function Walk($e, $d) {{ if ($d -gt 2) {{ return }}; $c = $w.GetFirstChild($e); \
           while ($c -ne $null -and $script:n -lt 150) {{ $script:n++; \
             [pscustomobject]@{{ depth = $d; name = $c.Current.Name; control_type = $c.Current.ControlType.ProgrammaticName -replace '^ControlType\\.', ''; automation_id = $c.Current.AutomationId }}; \
             Walk $c ($d + 1); $c = $w.GetNextSibling($c) }} }}; \
         @(Walk $root 0) | ConvertTo-Json -Compress"
    );
    let v = json(&script, 30).await?;
    Ok(as_list(v)
        .into_iter()
        .map(|e| UiElement {
            depth: e["depth"].as_u64().unwrap_or(0) as u32,
            name: e["name"].as_str().unwrap_or("").into(),
            control_type: e["control_type"].as_str().unwrap_or("").into(),
            automation_id: e["automation_id"].as_str().unwrap_or("").into(),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escaping_and_parsing() {
        assert_eq!(ps_str("it's"), "'it''s'");
        assert!(is_guid("381b4222-f694-41f0-9685-ff5bb260df2e"));
        assert!(!is_guid("381b4222-f694-41f0-9685-ff5bb260df2"));
        let ru = "Существующие схемы управления питанием (* Активный)\n-----------------------------------\n\
                  GUID схемы питания: 381b4222-f694-41f0-9685-ff5bb260df2e  (Сбалансированная) *\n\
                  GUID схемы питания: 8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c  (Высокая производительность)";
        let plans = parse_power_plans(ru);
        assert_eq!(plans.len(), 2);
        assert_eq!(plans[0], PowerPlan { guid: "381b4222-f694-41f0-9685-ff5bb260df2e".into(), name: "Сбалансированная".into(), active: true });
        assert!(!plans[1].active);
        assert_eq!(as_list(serde_json::json!({ "a": 1 })).len(), 1);
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn powershell_runs_and_reads_power_plans() {
        assert_eq!(run("Write-Output 'привет'", Duration::from_secs(30)).await.unwrap(), "привет");
        assert!(run("throw 'boom'", Duration::from_secs(30)).await.is_err());
        // Read-only: lists plans, changes nothing.
        let plans = power_plans().await.unwrap();
        assert!(plans.iter().filter(|p| p.active).count() <= 1);
    }
}
