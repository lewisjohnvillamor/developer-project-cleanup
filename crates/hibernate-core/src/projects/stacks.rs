//! Tables describing how each ecosystem is recognised.

use crate::model::Stack;

/// Node dependencies that identify a framework or flavour, in display
/// priority order. The first match becomes the project's primary label.
pub const NODE_FRAMEWORKS: &[(&str, &str)] = &[
    ("next", "Next.js"),
    ("nuxt", "Nuxt"),
    ("@sveltejs/kit", "SvelteKit"),
    ("astro", "Astro"),
    ("@remix-run/react", "Remix"),
    ("@angular/core", "Angular"),
    ("expo", "Expo"),
    ("react-native", "React Native"),
    ("electron", "Electron"),
    ("@tauri-apps/cli", "Tauri"),
    ("gatsby", "Gatsby"),
    ("vite", "Vite"),
    ("react", "React"),
    ("vue", "Vue"),
    ("svelte", "Svelte"),
    ("express", "Express"),
    ("fastify", "Fastify"),
    ("nest", "NestJS"),
    ("@nestjs/core", "NestJS"),
];

/// Files whose presence marks a directory as a project root.
pub const MARKERS: &[(&str, Stack)] = &[
    ("package.json", Stack::Node),
    ("Cargo.toml", Stack::Rust),
    ("pyproject.toml", Stack::Python),
    ("requirements.txt", Stack::Python),
    ("Pipfile", Stack::Python),
    ("poetry.lock", Stack::Python),
    ("setup.py", Stack::Python),
    ("go.mod", Stack::Go),
    ("pom.xml", Stack::Maven),
    ("build.gradle", Stack::Gradle),
    ("build.gradle.kts", Stack::Gradle),
    ("settings.gradle", Stack::Gradle),
    ("settings.gradle.kts", Stack::Gradle),
    ("Podfile", Stack::CocoaPods),
    ("pubspec.yaml", Stack::Dart),
    ("Gemfile", Stack::Ruby),
    ("composer.json", Stack::Php),
    ("Package.swift", Stack::Swift),
    ("mix.exs", Stack::Elixir),
    ("stack.yaml", Stack::Haskell),
    ("cabal.project", Stack::Haskell),
    ("build.zig", Stack::Zig),
];

/// File suffixes that mark a Haskell package.
pub const HASKELL_SUFFIXES: &[&str] = &[".cabal"];

/// File suffixes that mark a Terraform / OpenTofu root module.
pub const TERRAFORM_SUFFIXES: &[&str] = &[".tf"];

/// File suffixes that mark a .NET project or solution.
pub const DOTNET_SUFFIXES: &[&str] = &[".sln", ".csproj", ".fsproj", ".vbproj"];

/// Lockfile → Node package manager, checked in this order.
pub const NODE_LOCKFILES: &[(&str, &str)] = &[
    ("pnpm-lock.yaml", "pnpm"),
    ("yarn.lock", "yarn"),
    ("bun.lock", "bun"),
    ("bun.lockb", "bun"),
    ("package-lock.json", "npm"),
    ("npm-shrinkwrap.json", "npm"),
];

/// Lockfile → Python package manager, checked in this order.
pub const PYTHON_LOCKFILES: &[(&str, &str)] = &[
    ("uv.lock", "uv"),
    ("poetry.lock", "poetry"),
    ("Pipfile.lock", "pipenv"),
    ("Pipfile", "pipenv"),
    ("pdm.lock", "pdm"),
];
