import { ComponentType } from "react";
import {
  Activity,
  AppWindow,
  Bell,
  CalendarDays,
  CalendarRange,
  CloudSun,
  Clock,
  Clipboard,
  Cpu,
  GitBranch,
  Link2,
  ListTodo,
  LucideIcon,
  NotebookPen,
  Timer,
  TimerReset,
} from "lucide-react";
import { WidgetKind } from "../../lib/api";
import { BodyProps } from "./common";
import { AppLauncherWidget, NotesWidget, QuickLinksWidget, TodoWidget } from "./ProductivityWidgets";
import { ClipboardWidget, NotificationsWidget, StatusWidget, SystemInfoWidget, WeatherWidget } from "./InfoWidgets";
import { GitWidget } from "./DevWidgets";
import { CalendarWidget, ClockWidget, CountdownWidget, DateWidget, StopwatchWidget } from "./WidgetBodies";

/** UI side of each widget kind; the definitions themselves come from the backend catalog. */
export const WIDGET_UI: Record<WidgetKind, { icon: LucideIcon; Body: ComponentType<BodyProps> }> = {
  clock: { icon: Clock, Body: ClockWidget },
  date: { icon: CalendarDays, Body: DateWidget },
  calendar: { icon: CalendarRange, Body: CalendarWidget },
  countdown: { icon: TimerReset, Body: CountdownWidget },
  stopwatch: { icon: Timer, Body: StopwatchWidget },
  notes: { icon: NotebookPen, Body: NotesWidget },
  todo: { icon: ListTodo, Body: TodoWidget },
  quick_links: { icon: Link2, Body: QuickLinksWidget },
  app_launcher: { icon: AppWindow, Body: AppLauncherWidget },
  system_info: { icon: Cpu, Body: SystemInfoWidget },
  clipboard: { icon: Clipboard, Body: ClipboardWidget },
  notifications: { icon: Bell, Body: NotificationsWidget },
  weather: { icon: CloudSun, Body: WeatherWidget },
  status: { icon: Activity, Body: StatusWidget },
  git: { icon: GitBranch, Body: GitWidget },
};
