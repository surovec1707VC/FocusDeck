# Skills

Скилл — JSON-манифест в `skills/`. Скилл **не выполняет действий сам**: модель возвращает результат и *план* вызовов инструментов, а FocusDeck проверяет каждый шаг и решает, что показать пользователю на подтверждение. Во время тестового запуска план никогда не выполняется.

## Манифест
| Поле | Описание |
|---|---|
| `id` | kebab-case, совпадает с именем файла |
| `name`, `description`, `category` | для UI |
| `enabled` | значение по умолчанию; текущее состояние хранится в таблице `skills` |
| `required_permissions` | должны покрывать права всех `allowed_tools` |
| `allowed_tools` | инструменты из каталога `validator::TOOLS` |
| `system_prompt` | инструкция для модели |
| `input_schema` / `output_schema` | JSON Schema (`type`, `required`, `properties`, `items`, `enum`) |
| `safety_level` | `read_only` (только читающие инструменты) / `confirm_before_action` / `dangerous` (каждый шаг требует подтверждения) |
| `max_steps` | 1–20, лишние шаги плана отбрасываются |
| `model_profile` | один из AI-профилей |

Неизвестные поля запрещены, чтобы опечатки сразу находились при загрузке.

## Скиллы (14)
| id | safety | profile |
|---|---|---|
| project-context-builder | read_only | coding_assistant |
| prompt-improver | read_only | prompt_improver |
| task-planner | confirm_before_action | task_planner |
| code-reviewer | read_only | coding_assistant |
| error-explainer | read_only | error_explainer |
| test-generator | confirm_before_action | coding_assistant |
| refactor-planner | read_only | coding_assistant |
| automation-designer | confirm_before_action | task_planner |
| workflow-recovery | read_only | fast_assistant |
| daily-summary | confirm_before_action | fast_assistant |
| security-guard | read_only | coding_assistant |
| voice-intent-router | confirm_before_action | fast_assistant |
| widget-advisor | confirm_before_action | fast_assistant |
| knowledge-recall | read_only | local_private_assistant |

## Backend (`src-tauri/src/ai/skills*`)
- `loader.rs` — читает `skills/*.json`; битые файлы становятся «issues», остальные загружаются.
- `validator.rs` — каталог инструментов (право + признак «меняет состояние»), проверки манифеста, проверка JSON Schema.
- `executor.rs` — проверяет вход по `input_schema`, добавляет к промпту протокол ответа (`summary` / `result` / `plan`), вызывает модель через `AiProvider::chat`, проверяет `result` по `output_schema` и помечает шаги плана: `proposed` (с `needs_confirmation`) или `blocked` (инструмент не разрешён или неизвестен).
- `skills.rs` — `SkillRegistry` (синхронизация с таблицей `skills`, вкл/выкл, запуск, логи в `agent_tasks` / `agent_logs`) и команды: `skills_list`, `skills_issues`, `skills_reload`, `skills_set_enabled`, `skills_test_run`, `skills_logs`.
- Запуск невозможен, если скилл или модуль «Skills» в Control Center выключен.
- `skills/*.json` входят в бандл через `bundle.resources` в `tauri.conf.json`.

Инструменты пока только описаны в каталоге — их реального исполнения ещё нет. Исполнитель с подтверждением пользователя — следующий шаг.

## Frontend
- `src/pages/Skills.tsx` — список, поиск, фильтр по категории, вкл/выкл, перезагрузка манифестов, ошибки загрузки.
- `src/components/skills/SkillCard.tsx` — карточка.
- `src/components/skills/SkillSettings.tsx` — права и инструменты, схемы, system prompt, тестовый запуск (выбор провайдера и модели, JSON-вход, результат и план), логи запусков.

## Проверено
- `cargo test`: все 14 манифестов парсятся и проходят валидацию; тесты валидатора, JSON Schema и executor (план усечён до `max_steps`, недопустимые инструменты блокируются, ничего не выполняется) — с фейковым провайдером.
- В приложении: страница загружает все 14 скиллов; тестовый запуск без API-ключа сохраняется в логах с ошибкой об отсутствии API-ключа.
- **Не проверено:** запуск с реальной моделью.
