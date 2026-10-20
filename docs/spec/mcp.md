# Spec: MCP server (M7)

## Behavior

- `musiccraft-mcp` is a Model Context Protocol server speaking JSON-RPC 2.0 over
  stdio, one message per line. It holds one project session for its lifetime.
- `initialize` answers with the client's protocol version when supported
  (2025-06-18, 2025-03-26, 2024-11-05), otherwise the newest; it advertises the
  `tools` capability and short usage instructions.
- Every registry command (including `project.export_wav`, `edit.undo`,
  `edit.redo` and `edit.batch`) is a tool named with `_` for `.`
  (`note.add` → `note_add`). Input: `{"args": ["...", ...]}`; non-string values
  are converted to strings and validated by the command.
- Extra tools: `run` (one line), `run_script` (many lines, stops at the first
  error), `get_state` (readable summary), `get_project` (file-format JSON),
  `list_commands`, and `analyze_audio` (offline render; duration, peak, RMS, and
  per-bar levels, flagging clipping).
- Command failures are tool results with `isError: true`. Unknown tools and bad
  parameters are JSON-RPC errors (-32602); unknown methods -32601; malformed JSON
  -32700. Notifications get no reply.

## Command-line options

- `musiccraft-mcp` with no options serves its own headless session.
- `--dir <folder>`: run in that folder, so relative paths (`project.save
  song.mcproj`, `project.export_wav beat.wav`, samples) are read and written
  there. The folder must exist.
- `--bridge <host:port> --control-token-file <file>`: serve the session of an
  open MusicCraft window instead (live mode, below). The server only relays:
  every line from stdin goes to the window, every reply comes back on stdout.
  It exits when stdin closes or the window goes away.

## Live mode: the agent works in the open window

- `musiccraft --control <port> --control-token-file <file>` starts the desktop
  app with a control channel on `127.0.0.1:<port>` (localhost only). At start
  it writes a fresh random token (32 hex digits) to `<file>`, readable only by
  the user on Unix, writing a temporary file first and renaming it into place,
  so the file is complete once it exists. Without a token file the channel
  refuses to start (the app runs normally and says so in the status bar).
- `--dir <folder>` makes the app run in that folder, as for the MCP server.
- A client sends the token as its first line; a wrong token closes the
  connection. Every following line is one JSON-RPC message (or batch), handled
  exactly as the headless server handles it, but against the window's project:
  the user sees each change as it happens, undo works across both, and the
  user can keep working.
- Commands run through the same path as the interface (plugin state is kept,
  saving marks the project clean). Commands that replace the project
  (`project.new`, `project.load`, `project.recover`,
  `project.import_flp`) are refused with a tool
  error while the window has unsaved changes, so an agent never discards the
  user's work; the agent saves first or asks the user.
- The `initialize` instructions say the agent is working in the user's open
  window.
- Several clients may connect; their requests are handled one at a time on the
  interface thread, between frames.

## Client configuration

```json
{ "mcpServers": { "musiccraft": { "command": "/path/to/target/release/musiccraft-mcp" } } }
```

## Sources

- The Model Context Protocol specification (open standard, modelcontextprotocol.io)
  and JSON-RPC 2.0. Tool set: own design.
- Live mode and its options: own design, matching the arguments CreativeCraft
  (an open-source app hub) passes to the apps it drives (`catalog/apps.toml` in
  its repository).
