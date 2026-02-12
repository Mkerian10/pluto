# The Pluto Programming Language Book

This is the official documentation for the Pluto programming language, built with [mdBook](https://rust-lang.github.io/mdBook/).

## 📖 Reading Online

The book is automatically deployed to GitHub Pages on every push to `master`:

**https://mkerian10.github.io/pluto/**

## 🛠️ Building Locally

To build and serve the book locally:

```bash
# Install mdBook (if not already installed)
cargo install mdbook

# Build the book
mdbook build

# Serve the book locally (with live reload)
mdbook serve
```

The book will be available at `http://localhost:3000`.

## 📝 Contributing

The book source files are in `src/`. The structure is defined in `src/SUMMARY.md`.

To add a new page:
1. Create a new `.md` file in the appropriate `src/` subdirectory
2. Add an entry to `src/SUMMARY.md`
3. Build and test locally with `mdbook serve`

## 🚀 Deployment

The book is automatically deployed to GitHub Pages via the `.github/workflows/deploy-book.yml` workflow. Every push to `master` triggers a new deployment.

## 📋 Structure

```
book/
├── book.toml              # mdBook configuration
├── src/                   # Markdown source files
│   ├── SUMMARY.md        # Table of contents
│   ├── introduction.md   # Introduction
│   ├── getting-started/  # Getting started guide
│   ├── whats-different/  # What sets Pluto apart
│   ├── language/         # Language reference
│   └── stdlib/           # Standard library docs
└── book/                 # Generated output (gitignored)
```
