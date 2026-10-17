// Generated from the Rust types by `pnpm gen:types` (src-tauri/src/ipc/bindings.rs). Do not edit:
// change the Rust type, regenerate, and commit both. `pnpm check` fails while this file is out of date.

export type CatState = { awake: boolean, reason: WakeReason | null,
/**
 * Sessions currently calling for you, oldest first.
 */
sessions: Array<SessionView>, attention: Attention | null,
/**
 * Ready is armed: finished turns wake the cat until you turn it off.
 */
ready: boolean,
/**
 * Why the latest change happened; read by the overlay when the cat goes from awake to asleep.
 */
sleepCause: SleepCause, };

export type SessionView = { projectName: string, reason: WakeReason, };

/**
 * The latest "a session needs you" moment. `seq` changes on each one, so the overlay can tell a new
 * session (or a new reason) that arrives while the cat is already awake and yell again.
 */
export type Attention = { seq: number, projectName: string, reason: WakeReason, };

export type WakeReason = "done" | "permission" | "input";

/**
 * Why the last change happened, so the overlay can pick how the cat falls asleep: knocked out by a flying
 * object when you did something (answered Claude, dismissed, turned Ready off), a yawn when nobody did
 * (a waiting session expired, notifications were paused).
 */
export type SleepCause = "user" | "auto";

export type Settings = { character: Character, notificationsEnabled: boolean, hideWhileSleeping: boolean, sfwMode: boolean, uiLanguage: UiLanguage, klatLanguage: KlatLanguage, yellIntervalMs: DurationMs, maxYellsPerWake: number, staleAfterMs: DurationMs, reduceMotion: boolean,
/**
 * Size of the cat in percent (the whole overlay: cat, bubble and Ready button).
 */
catScale: number, startAtLogin: boolean, corner: Corner,
/**
 * Where you dragged the cat (window top-left, physical px); None = the corner.
 */
position: Point | null, };

/**
 * What the Settings window may change (`set_settings`). Not `position` (dragging sets it), `corner` (one
 * value) or `startAtLogin` (it must go through the OS, see `autostart`): those keys are ignored, as are keys
 * this version does not know, so a newer or older Settings window never fails to save.
 *
 * Read like the file: a value of the wrong type, `null` or an unknown name becomes that field's default, so
 * the patch resets the field rather than failing. `None` = the key was not in the patch.
 */
export type SettingsPatch = { character?: Character, notificationsEnabled?: boolean, hideWhileSleeping?: boolean, sfwMode?: boolean, uiLanguage?: UiLanguage, klatLanguage?: KlatLanguage, yellIntervalMs?: DurationMs, maxYellsPerWake?: number, staleAfterMs?: DurationMs, reduceMotion?: boolean, catScale?: number, };

/**
 * Who sits in the corner (src/characters in the frontend). The overlay draws it; Rust only stores the choice.
 */
export type Character = "cat" | "snake" | "dragon";

/**
 * Platform Language: the tray, the Settings window and dialogs.
 */
export type UiLanguage = "en" | "vi";

/**
 * Klat Language: what the cat says. Auto mixes every language.
 */
export type KlatLanguage = "en" | "vi" | "zh" | "ja" | "ko" | "auto";

/**
 * Where the cat sits until you drag it. Only one corner so far.
 */
export type Corner = "bottom-right";

/**
 * A length of time, in milliseconds.
 */
export type DurationMs = number;

/**
 * A text from the shared catalogs (src/i18n) and the values of its `{placeholders}`.
 */
export type Message = { key: string, vars: { [key in string]: string }, };

/**
 * A command's failure: what failed and what to tell the user.
 */
export type IpcError = { code: ErrorCode, message: Message, };

/**
 * What failed, for a window that wants to react to a kind of failure rather than show the text.
 */
export type ErrorCode = "hooks-not-found" | "hooks-broken-json" | "hooks-unexpected-shape" | "hooks-exe-missing" | "hooks-changed-concurrently" | "hooks-io" | "start-at-login" | "window";

export type HooksInfo = { agent: HooksAgent, status: HooksStatus,
/**
 * `~/.claude/settings.json` or `~/.codex/hooks.json`; None when the home folder is unknown.
 */
filePath: string | null,
/**
 * The agent's folder (`~/.claude`, `~/.codex`) exists, so it looks set up for this user. Install works
 * either way.
 */
agentFound: boolean, };

/**
 * The coding agents whose hooks Klat installs. Both spell their hooks the same way and send the same fields
 * (`session_id`, `cwd`, `hook_event_name`, `tool_name`), so one adapter serves both; Codex entries tell it
 * which agent runs it (`klat-hook --agent codex Stop`).
 */
export type HooksAgent = "claude-code" | "codex";

export type HooksStatus = "not-installed" | "needs-repair" | "installed";

export type HooksAction = "install" | "uninstall";

/**
 * A point `{x, y}`, in whatever unit the field or event says.
 */
export type Point = { x: number, y: number, };

/**
 * A rectangle `{x, y, w, h}`: top-left corner, width and height, in whatever unit the field or event says.
 */
export type Rect = { x: number, y: number, w: number, h: number, };

/**
 * Moving the cat with Space: Space held over the cat (`Armed`), then the mouse pressed (`Dragging`).
 */
export type GrabMode = "idle" | "armed" | "dragging";

/**
 * What `preview_zoom` did, for the page to know when the switch has landed.
 */
export type ZoomChange = { from: number, to: number,
/**
 * The page's size in CSS px, the same at every zoom.
 */
width: number, height: number, };

/**
 * Whether the Space hook may run. Only macOS asks (Accessibility); elsewhere it is not needed.
 */
export type DragPermission = "not-needed" | "granted" | "missing";
