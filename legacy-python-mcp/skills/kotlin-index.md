# kotlin-index - Code Search for Android/Kotlin/Java Projects

Fast code search tool for Android/Kotlin/Java projects using SQLite + FTS5.

## Prerequisites

Install the CLI:
```bash
# macOS (recommended)
brew tap defendend/kotlin-index-
brew install kotlin-index

# or via pip
pip install kotlin-index
```

Initialize index in project root:
```bash
cd /path/to/android/project
kotlin-index init
```

## Available Commands (33 total)

### Search Commands

**Universal search** (files + symbols + modules):
```bash
kotlin-index search "PaymentMethod"
```

**Find files by name**:
```bash
kotlin-index file "Fragment.kt"
kotlin-index file --exact "PaymentMethodsFragment.kt"
```

**Find symbols** (classes, interfaces, functions):
```bash
kotlin-index symbol "PaymentInteractor"
kotlin-index symbol --type class "Repository"
kotlin-index symbol --type function "validate"
```

**Find class/interface**:
```bash
kotlin-index class "BaseFragment"
```

**Find usages** of a symbol:
```bash
kotlin-index usages "PaymentRepository"
```

**Find implementations** (subclasses/implementors):
```bash
kotlin-index implementations "BasePresenter"
```

**Class hierarchy** (parents + children):
```bash
kotlin-index hierarchy "BaseFragment"
```

**Find callers** of a function:
```bash
kotlin-index callers "onClick"
```

**File imports**:
```bash
kotlin-index imports "path/to/File.kt"
```

### DI Commands (Dagger)

**Find @Provides/@Binds** for a type:
```bash
kotlin-index provides "UserRepository"
```

**Find @Inject** points for a type:
```bash
kotlin-index inject "UserInteractor"
```

**Find classes with annotation**:
```bash
kotlin-index annotations "@Module"
kotlin-index annotations "@Inject"
```

### Audit Commands

**Find TODO/FIXME/HACK**:
```bash
kotlin-index todo
kotlin-index todo "FIXME"
```

**Find @Deprecated** items:
```bash
kotlin-index deprecated
```

**Find @Suppress** annotations:
```bash
kotlin-index suppress
kotlin-index suppress "UNCHECKED_CAST"
```

**Find extension functions**:
```bash
kotlin-index extensions "String"
kotlin-index extensions "View"
```

**Show public API** of a module:
```bash
kotlin-index api "features/payments/api"
```

**Find deeplinks**:
```bash
kotlin-index deeplinks
kotlin-index deeplinks "payment"
```

**Show changed symbols** (git diff):
```bash
kotlin-index changed
kotlin-index changed --base "origin/main"
```

### Compose Commands

**Find @Composable functions**:
```bash
kotlin-index composables
kotlin-index composables "Button"
```

**Find @Preview functions**:
```bash
kotlin-index previews
```

### Coroutines Commands

**Find suspend functions**:
```bash
kotlin-index suspend
kotlin-index suspend "fetch"
```

**Find Flow/StateFlow/SharedFlow**:
```bash
kotlin-index flows
kotlin-index flows "user"
```

### Module Commands

**Find modules**:
```bash
kotlin-index module "payments"
```

**Module dependencies**:
```bash
kotlin-index deps "features.payments.impl"
```

**Modules depending on this module**:
```bash
kotlin-index dependents "features.payments.api"
```

### File Structure

**File outline** (classes, functions in file):
```bash
kotlin-index outline "path/to/File.kt"
```

### Index Management

**Rebuild index** (full):
```bash
kotlin-index rebuild
kotlin-index rebuild --type files
kotlin-index rebuild --type symbols
kotlin-index rebuild --type modules
```

**Update index** (incremental):
```bash
kotlin-index update
```

**Index statistics**:
```bash
kotlin-index stats
```

## Environment Variables

- `KOTLIN_INDEX_PROJECT_ROOT` - project root (auto-detected)
- `KOTLIN_INDEX_DB_PATH` - database path (default: `~/.cache/kotlin-index/index.db`)

## Performance Tips

- Install ripgrep (`brew install ripgrep`) for 10-15x faster grep-based commands
- Use `kotlin-index update` for incremental updates after file changes
- Symbol types: `class`, `interface`, `object`, `function`, `property`, `enum`
