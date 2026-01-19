# kotlin-index MCP Server

MCP сервер для быстрого поиска по Android/Kotlin проектам. Индексирует файлы, символы (классы, функции, интерфейсы) и Gradle модули с зависимостями.

## Возможности

- **Поиск файлов** — по имени или части пути
- **Поиск символов** — классы, интерфейсы, функции, свойства с фильтрацией по типу
- **Структура файла** — outline с номерами строк
- **Модули и зависимости** — парсинг build.gradle, граф зависимостей
- **Быстрый поиск** — SQLite + FTS5, миллисекунды на запрос

## Установка

### 1. Клонирование

```bash
git clone https://github.com/defendend/Claude-index-search-android-studio.git .claude/mcp-index
```

### 2. Зависимости

```bash
cd .claude/mcp-index
python3 -m venv .venv
source .venv/bin/activate
pip install -r requirements.txt
```

### 3. Регистрация MCP

Создайте `.mcp.json` в корне проекта:

```json
{
  "mcpServers": {
    "kotlin-index": {
      "type": "stdio",
      "command": "sh",
      "args": ["/path/to/project/.claude/mcp-index/mcp_server.sh"],
      "env": {}
    }
  }
}
```

### 4. Исключение из Git

```bash
echo ".mcp.json" >> .git/info/exclude
```

### 5. Перезапуск Claude Code

После добавления `.mcp.json` перезапустите Claude Code.

## Инструменты

### Поиск файлов

| Инструмент | Описание |
|------------|----------|
| `find_file(query, limit=20)` | Поиск файлов по имени или части пути |
| `find_file_exact(name)` | Найти файл по точному имени |

```
find_file("UserRepository")         → список файлов
find_file_exact("MainActivity.kt")  → полный путь
```

### Поиск символов

| Инструмент | Описание |
|------------|----------|
| `find_symbol(query, symbol_type?, limit=20)` | Поиск по имени с фильтром типа |
| `find_class(name)` | Найти класс/интерфейс |
| `get_file_outline(file_path)` | Структура файла |

**Типы символов:**
- `class` — классы
- `interface` — интерфейсы
- `object` — Kotlin objects
- `function` — функции
- `property` — свойства (val/var)
- `enum` — enum классы

```
find_symbol("Presenter", "class")      → классы с "Presenter"
find_symbol("onCreate", "function")    → функции onCreate
find_class("MainViewModel")            → путь и строка
get_file_outline("/path/to/File.kt")   → структура файла
```

### Модули и зависимости

| Инструмент | Описание |
|------------|----------|
| `find_module(query, limit=20)` | Поиск модулей |
| `get_module_deps(module_name)` | Зависимости модуля |
| `get_module_dependents(module_name)` | Кто зависит от модуля |

```
find_module("network")                    → модули с "network"
get_module_deps("features.auth.impl")     → зависимости
get_module_dependents("core.network.api") → dependents
```

### Универсальный поиск

| Инструмент | Описание |
|------------|----------|
| `search(query, limit=10)` | Поиск по файлам, символам и модулям |

### Управление индексом

| Инструмент | Описание |
|------------|----------|
| `rebuild_index(type="all")` | Пересобрать индекс |
| `get_index_stats()` | Статистика |

**Типы:**
- `files` — только файлы
- `modules` — модули и зависимости
- `symbols` — символы (классы, функции)
- `all` — всё

## Первый запуск

После установки создайте индекс:

```
rebuild_index("all")
```

## Когда обновлять

- После `git pull` / `git checkout`
- После добавления новых файлов/модулей
- После значительных изменений кода

## Конфигурация

| Переменная | Описание |
|------------|----------|
| `KOTLIN_INDEX_PROJECT_ROOT` | Корень проекта (автоопределение) |
| `KOTLIN_INDEX_DB_PATH` | Путь к БД (`~/.cache/kotlin-index/index.db`) |

## Архитектура

```
mcp-index/
├── server.py              # MCP сервер (FastMCP)
├── mcp_server.sh          # Скрипт запуска
├── requirements.txt       # Зависимости
├── db/
│   ├── database.py        # SQLite
│   └── schema.py          # Схема БД
└── indexer/
    ├── file_indexer.py    # Индексация файлов
    ├── module_indexer.py  # Парсинг build.gradle
    └── symbol_indexer.py  # Парсинг Kotlin (tree-sitter)
```

## Технологии

- **FastMCP** — MCP фреймворк
- **SQLite + FTS5** — полнотекстовый поиск
- **tree-sitter-kotlin** — парсинг AST

## Производительность

| Операция | Время |
|----------|-------|
| Полная индексация | ~60 сек* |
| Поиск | < 100 мс |

*Зависит от размера проекта

## Git Worktrees

Для работы с worktrees создайте `.mcp.json` в каждом worktree с соответствующим путём к `mcp_server.sh`.

## Troubleshooting

### "too many SQL variables"
Операции разбиты на батчи в `db/database.py`.

### Модули = 0
Проверьте фильтр файлов в `module_indexer.py`.

### Символы не находятся по типу
Проверьте типы узлов tree-sitter в `symbol_indexer.py`.

## Лицензия

MIT
