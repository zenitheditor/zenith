# @zenitheditor/zenith-mcp

**Design as code for AI agents and humans.** NPM launcher for the [Zenith](https://github.com/zenitheditor/zenith) MCP server.

Let your coding agent create, validate, and edit plain-text `.zen` design documents — then inspect and polish the exact same files visually in the **[Zenith browser editor](https://zenitheditor.github.io/zenith/)** or render deterministic PNG, SVG, and print-ready PDF.

This package installs the matching prebuilt `zenith` binary from GitHub Releases and exposes a
`zenith-mcp` command for MCP clients.

## Usage

Run the MCP server:

```bash
npx -y @zenitheditor/zenith-mcp
```

Pass arguments through to the underlying `zenith` binary:

```bash
npx -y @zenitheditor/zenith-mcp --help
```

Install globally:

```bash
npm install -g @zenitheditor/zenith-mcp
zenith-mcp
```

## MCP Registry

MCP Registry name: `mcp-name: io.github.zenitheditor/zenith`

The main project is at https://github.com/zenitheditor/zenith.
