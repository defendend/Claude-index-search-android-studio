#!/bin/bash

# Определяем директорию скрипта
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Определяем корень проекта (на 2 уровня выше от .claude/mcp-index)
# Поддерживаем оба worktree
if [[ "$SCRIPT_DIR" == *"go-client-android-2"* ]]; then
    PROJECT_ROOT="/Users/defendend/go-client-android-2"
else
    PROJECT_ROOT="/Users/defendend/go-client-android"
fi

# Путь к БД (общий для обоих worktree, чтобы не индексировать дважды)
DB_PATH="$HOME/.cache/go-index/index.db"

# Создаём директорию для БД
mkdir -p "$(dirname "$DB_PATH")"

# Активируем venv если есть
if [ -d "$SCRIPT_DIR/.venv" ]; then
    source "$SCRIPT_DIR/.venv/bin/activate"
fi

# Экспортируем переменные
export GO_INDEX_PROJECT_ROOT="$PROJECT_ROOT"
export GO_INDEX_DB_PATH="$DB_PATH"

# Запускаем сервер
exec python3 "$SCRIPT_DIR/server.py"
