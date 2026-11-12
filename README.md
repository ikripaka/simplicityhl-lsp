# SimplicityHL LSP

This project was originally part of [SimplicityHL](https://github.com/BlockstreamResearch/SimplicityHL), the high-level language for writing Simplicity smart contracts.

Language Server for [SimplicityHL language](https://simplicity-lang.org/).

## Features

- Basic diagnostic for SimplicityHL code

![diagnostics](assets/diagnostics.gif)

- Completions of built-ins, jets and functions, plus context-aware `use` paths and
  public importable items

![completion](assets/completion.gif)

- Hover for built-ins, jets and functions, with support of documentation

![hover](assets/hover.gif)

- Go to definition for functions

![goto-definition](assets/goto-definition.gif)

- Document formatting with [`simfmt`](https://github.com/BlockstreamResearch/simfmt), the formatter behind `simplex fmt`

- Semantic highlighting for raw Simplicity (`.simpl`) files, including unsaved and
  incomplete source. See the [first example](#raw-simplicity-example).

## Raw Simplicity example

[`examples/verify_equal.simpl`](examples/verify_equal.simpl) is a small program that
checks equality of two 32-bit constants. It demonstrates comments, named expressions,
combinators, constants, jets, and type annotations. The protocol tests parse and
type-check this same file with `Forest::parse::<Elements>()`, then check highlighting
through a running language server.

For local VS Code development, use checkouts of `simplicityhl-lsp` and
`simplicityhl-vscode`.

1. Run `cargo build --locked` in the LSP checkout.
2. Run `npm ci` and `npm run compile` in the extension checkout.
3. In a separate VS Code window, set `simplicityhl.server.path` to the absolute path
   of `simplicityhl-lsp/target/debug/simplicityhl-lsp` (`.exe` on Windows). For an
   isolated setup, use a temporary `.code-workspace` file with these settings:

   ```json
   {
     "folders": [{ "path": "/absolute/path/to/simplicityhl-lsp" }],
     "settings": {
       "simplicityhl.server.path": "/absolute/path/to/simplicityhl-lsp/target/debug/simplicityhl-lsp",
       "simplicityhl.disableAutoupdate": true
     }
   }
   ```

4. Launch that workspace with the local extension:

   ```sh
   code --new-window --extensionDevelopmentPath=/absolute/path/to/simplicityhl-vscode /absolute/path/to/example.code-workspace
   ```

5. Open `examples/verify_equal.simpl`. The language mode should be **Simplicity**.
   Use **Developer: Inspect Editor Tokens and Scopes** to inspect a jet (`function`),
   a combinator (`keyword`), or a named expression (`variable`); colors depend on the
   theme. Insert a blank line or an unfinished `(` without saving and confirm that
   highlighting follows the buffer. Undo the edits, close, and reopen the file.
6. Temporarily set `"[simplicity]": { "editor.semanticHighlighting.enabled": false }`
   in the workspace settings to check the extension's TextMate highlighting, then
   remove that override to restore semantic highlighting.

This initial `.simpl` support is highlighting only. Diagnostics, completion,
compilation commands, witness execution, and calling raw Simplicity from SimplicityHL
are not implemented. A misspelled jet or unequal constants will not produce an editor
error. Syntax coloring does not imply that the program is valid or that it succeeds.

## Configuration

Editors send settings under the `simplicityhl` section. Every field is optional; the
defaults below are what the server uses when a client sends nothing.

```json
{
  "simplicityhl": {
    "experimentalFeatures": {
      "imports": false,
      "enums": false
    },
    "project": {
      "simplex": {
        "enabled": true,
        "manifestPath": ""
      },
      "sourceDirectory": "",
      "dependencies": {}
    },
    "formatter": {
      "path": ""
    }
  }
}
```

- `experimentalFeatures.imports` enables the compiler's unstable `use` / `mod` / `pub`
  syntax. It is off by default because the feature is unstable in the compiler itself.
- `experimentalFeatures.enums` enables enum declarations and enum match patterns,
  likewise unstable in the compiler.
- `project.simplex.enabled` looks for the nearest `Simplex.toml` in the file's ancestors
  and honours its `build.src_dir` and `[dependencies]`, resolving path dependencies
  recursively and locating installed git dependencies under `deps/`. Like `simplex`, it
  only reads a file named exactly `Simplex.toml`.
- `project.simplex.manifestPath` pins an explicit manifest instead of searching. Relative
  values resolve from the containing workspace folder. A path that does not exist is
  reported as a diagnostic rather than silently ignored.
- `project.sourceDirectory` overrides the package root when there is no manifest, or when
  the manifest's `src_dir` is not what the editor should use.
- `project.dependencies` adds import roots by hand. They supplement the manifest and win
  on a colliding alias:

```json
{
  "std": "../simplicityhl-std/simf",
  "math": { "path": "../math/simf", "context": "simf/contracts" }
}
```

  The shorthand form maps an alias for the whole package. The detailed form restricts the
  mapping to files under `context`, matching how the compiler scopes dependencies per
  package.

- `formatter.path` sets the `simfmt` binary used for formatting. If empty, the server
  looks for `simfmt` next to `simplex` on `PATH`, then in `~/.simplex/bin` (or
  `$SIMPLEX_DIR/bin`), then anywhere on `PATH`. The nearest `simfmt.toml` applies, same
  as with `simplex fmt`.

Configuration changes take effect immediately: the server re-analyses open documents on
`workspace/didChangeConfiguration`, on workspace-folder changes, and when a watched
`.simf` file or Simplex manifest changes on disk.

## Installation

Install Language Server using `cargo`:

```bash
cargo install simplicityhl-lsp
```

## Integration with editors

### Neovim

#### LSP

0. Install `simplicityhl-lsp` to your `PATH`.

1. Include this Lua snippet to your Neovim config:

```lua
vim.filetype.add({
  extension = {
    simf = "simf",
  },
})

vim.lsp.config["simplicityhl-lsp"] = {
  cmd = { "simplicityhl-lsp" },
  filetypes = { "simf" },
  settings = {
    simplicityhl = {
      experimentalFeatures = { imports = true, enums = false },
      project = { simplex = { enabled = true, manifestPath = "" } },
    },
  },
}
vim.lsp.enable("simplicityhl-lsp")
```

2. Open `.simf` file and check that LSP is active ("attached"):

```vim
:checkhealth vim.lsp
```

#### Tree-sitter (Highlighting)

For broader syntax highlighting of SimplicityHL (`.simf`) alongside the language
server's semantic tokens, you can install tree-sitter:

0. Set up the [`nvim-treesitter`](https://github.com/nvim-treesitter/nvim-treesitter/tree/main) plugin.

1. Include this Lua snippet in your Neovim config to register parser:

```lua
vim.api.nvim_create_autocmd("User", {
  pattern = "TSUpdate",
  callback = function()
    require("nvim-treesitter.parsers").simplicityhl = {
      install_info = {
        url = "https://github.com/distributed-lab/tree-sitter-simplicityhl",
        queries = "queries",
      },
      filetype = "simf",
      tier = 0,
    }
  end,
})

vim.treesitter.language.register("simplicityhl", { "simf" })

vim.api.nvim_create_autocmd("FileType", {
  pattern = { "simf" },
  callback = function()
    vim.treesitter.start()
  end,
})
```

2. Restart Neovim and run:

```vim
:TSInstall simplicityhl
```

If everything is working correctly, you should see syntax highlighting in `.simf` files.

**Note:** This method is compatible only with `nvim-treesitter` v0.10 or newer.
