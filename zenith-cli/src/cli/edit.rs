//! Argument types for `zenith edit`.

use clap::Args;
use std::path::PathBuf;

/// Arguments for `zenith edit`.
#[derive(Debug, Args)]
#[command(
    after_help = "ROUTES (every /api route needs 'Authorization: Bearer <token>'):\n  \
GET  /#token=<t>          the editor page. The page reads the token from the fragment\n  \
POST /api/cmd             {command, params?, version?, diff?} -> {ok, version, result|error, image?, diff?}\n  \
POST /api/save            {version?, overwrite?} -> file.save envelope\n  \
GET  /api/state[?text=1]  {path, version, dirty, valid, conflict, ...}\n  \
GET  /api/events          Server-Sent Events: state, session, external_change, saved, stopping, shutdown\n  \
GET  /api/image/<sha>.png a render from doc.render or gesture.preview\n  \
POST /api/shutdown        {force?}. 409 while unsaved edits remain\n\n\
Ctrl-C with unsaved edits warns the page and waits. A second Ctrl-C discards them (exit 130).\n\
Agents send 'Authorization: Bearer <token>' and 'Content-Type: application/json'.\n\
Commands: send {\"command\":\"commands.list\"}. Host commands: file.save, file.reload, file.state.\n\n\
EXAMPLES:\n  \
zenith edit poster.zen                      # serve on a free loopback port, open the browser\n  \
zenith edit poster.zen --no-open --json     # print {url, port, token} as JSON for an agent\n  \
curl -s -XPOST http://127.0.0.1:PORT/api/cmd -H \"Authorization: Bearer $TOKEN\" \\\n    \
-H 'Content-Type: application/json' -d '{\"command\":\"doc.outline\"}'"
)]
pub struct EditArgs {
    /// Path to the `.zen` document.
    pub path: PathBuf,

    /// Port to listen on. Default: a free port, printed at start.
    #[arg(long, value_name = "N")]
    pub port: Option<u16>,

    /// Address to bind: an IP address or `localhost`.
    #[arg(long, value_name = "ADDR", default_value = "127.0.0.1")]
    pub host: String,

    /// Allow a non-loopback `--host`. The traffic is plain HTTP and anyone
    /// who reaches it can try the token: prefer an SSH tunnel or a TLS proxy.
    #[arg(long)]
    pub allow_remote: bool,

    /// Directory the editor may read project files from. Default: the
    /// document's directory. It must contain the document.
    #[arg(long, value_name = "DIR")]
    pub root: Option<PathBuf>,

    /// Do not open the browser.
    #[arg(long)]
    pub no_open: bool,

    /// Print the start line as JSON: {schema, url, host, port, token, path}.
    #[arg(long)]
    pub json: bool,
}
