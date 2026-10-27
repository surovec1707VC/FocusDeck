import { Music, Pause, Play, SkipBack, SkipForward } from "lucide-react";
import { t } from "../../i18n";
import { EVENTS, NowPlaying } from "../../lib/api";
import { useModules } from "../../lib/modules-context";
import { useWidgetData } from "../../lib/widget-data";
import { BodyProps, DataStatus, IconButton, Muted, PermissionNote } from "./common";

interface MediaData {
  permission: boolean;
  now?: NowPlaying | null;
}

/** Bars animated in CSS (`.equalizer` in index.css); frozen while paused or with reduced motion. */
function Equalizer({ playing }: { playing: boolean }) {
  return (
    <div className={`equalizer ${playing ? "is-playing" : ""}`} aria-hidden>
      {Array.from({ length: 5 }, (_, i) => (
        <span key={i} />
      ))}
    </div>
  );
}

export function MediaWidget({ widget }: BodyProps) {
  const W = t.widgets;
  const s = useModules().settingsOf("widget_media");
  // The backend watcher emits on every track/state change.
  const { data, meta, error, act, loading } = useWidgetData<MediaData>(widget, { events: [EVENTS.mediaChanged] });

  if (data && !data.permission) return <PermissionNote name="Медиа: что сейчас играет" />;
  const now = data?.now;
  if (!now?.active) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-2 text-center">
        <Music className="size-6 text-zinc-400" />
        <Muted>{W.mediaNothing}</Muted>
        <DataStatus meta={meta} error={error} />
      </div>
    );
  }

  const playing = now.status === "playing";
  const canToggle = playing ? now.can_pause : now.can_play;
  const iconSize = "size-4";
  return (
    <div className="flex h-full flex-col gap-2">
      <div className="flex min-h-0 flex-1 items-center gap-3">
        {s.show_cover !== false && (
          <div className="relative aspect-square h-full max-h-24 min-h-12 shrink-0 overflow-hidden rounded-lg bg-zinc-200 dark:bg-zinc-800">
            {now.cover ? (
              <img src={now.cover} alt="" className="size-full object-cover" draggable={false} />
            ) : (
              <Music className="absolute inset-0 m-auto size-6 text-zinc-400" />
            )}
          </div>
        )}
        <div className="min-w-0 flex-1">
          <div className="truncate font-semibold" title={now.title}>
            {now.title || W.mediaUnknown}
          </div>
          {now.artist && (
            <div className="truncate text-xs text-zinc-500" title={now.artist}>
              {now.artist}
            </div>
          )}
          <div className="mt-1 flex items-center gap-2 text-xs text-zinc-500">
            {s.show_equalizer !== false && <Equalizer playing={playing} />}
            {s.show_source !== false && <span className="truncate">{W.mediaFrom(now.source)}</span>}
            {!playing && <span className="shrink-0">· {W.mediaPaused}</span>}
          </div>
        </div>
      </div>
      <div className="flex shrink-0 items-center justify-center gap-2">
        <IconButton label={W.mediaPrevious} onClick={() => act("previous")} disabled={!now.can_previous || loading}>
          <SkipBack className={iconSize} />
        </IconButton>
        <button
          type="button"
          className="rounded-full bg-accent-600 p-2 text-white transition hover:bg-accent-500 disabled:opacity-40"
          onClick={() => act("play_pause")}
          disabled={!canToggle}
          title={playing ? W.mediaPause : W.mediaPlay}
          aria-label={playing ? W.mediaPause : W.mediaPlay}
        >
          {playing ? <Pause className={iconSize} /> : <Play className={iconSize} />}
        </button>
        <IconButton label={W.mediaNext} onClick={() => act("next")} disabled={!now.can_next || loading}>
          <SkipForward className={iconSize} />
        </IconButton>
      </div>
      <DataStatus meta={meta} error={error} />
    </div>
  );
}
