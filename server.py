#!/usr/bin/env python3
"""
MCP сервер для индексации и поиска по Android проекту.

Предоставляет быстрый поиск:
- Файлов по имени
- Символов (классы, функции, интерфейсы)
- Модулей и их зависимостей
"""

import os
import sys
import time
from pathlib import Path
from typing import Optional

# Добавляем текущую директорию в путь для импортов
SCRIPT_DIR = Path(__file__).parent
sys.path.insert(0, str(SCRIPT_DIR))

from fastmcp import FastMCP
from db.database import Database
from indexer.file_indexer import FileIndexer
from indexer.module_indexer import ModuleIndexer
from indexer.symbol_indexer import SymbolIndexer

# Конфигурация
PROJECT_ROOT = os.environ.get("GO_INDEX_PROJECT_ROOT", "/Users/defendend/go-client-android")
DB_PATH = os.environ.get("GO_INDEX_DB_PATH", str(SCRIPT_DIR / "index.db"))

# Инициализация
mcp = FastMCP("go-index")
db = Database(DB_PATH)
db.connect()

file_indexer = FileIndexer(db, PROJECT_ROOT)
module_indexer = ModuleIndexer(db, PROJECT_ROOT)
symbol_indexer = SymbolIndexer(db, PROJECT_ROOT)


# === Инструменты поиска файлов ===

@mcp.tool()
def find_file(query: str, limit: int = 20) -> str:
    """
    Поиск файлов по имени или части пути.

    Args:
        query: Строка поиска (имя файла или часть пути)
        limit: Максимальное количество результатов (по умолчанию 20)

    Returns:
        JSON со списком найденных файлов
    """
    results = db.search_files(query, limit)

    if not results:
        return f"Файлы по запросу '{query}' не найдены"

    output = [f"Найдено файлов: {len(results)}"]
    for f in results:
        output.append(f"  {f['path']}")

    return "\n".join(output)


@mcp.tool()
def find_file_exact(name: str) -> str:
    """
    Найти файл по точному имени.

    Args:
        name: Точное имя файла (например, PaymentMethodsFragment.kt)

    Returns:
        Полный путь к файлу или сообщение что не найден
    """
    results = db.execute(
        "SELECT path FROM files WHERE name = ? LIMIT 10",
        (name,)
    ).fetchall()

    if not results:
        return f"Файл '{name}' не найден"

    if len(results) == 1:
        return results[0]["path"]

    output = [f"Найдено {len(results)} файлов с именем '{name}':"]
    for r in results:
        output.append(f"  {r['path']}")
    return "\n".join(output)


# === Инструменты поиска символов ===

@mcp.tool()
def find_symbol(query: str, symbol_type: Optional[str] = None, limit: int = 20) -> str:
    """
    Поиск символов (классов, интерфейсов, функций) по имени.

    Args:
        query: Имя символа или его часть
        symbol_type: Тип символа (class, interface, object, function, property, enum). Опционально.
        limit: Максимальное количество результатов

    Returns:
        JSON со списком найденных символов
    """
    results = db.search_symbols(query, symbol_type, limit)

    if not results:
        type_str = f" типа '{symbol_type}'" if symbol_type else ""
        return f"Символы{type_str} по запросу '{query}' не найдены"

    output = [f"Найдено символов: {len(results)}"]
    for s in results:
        sig = f" {s['signature']}" if s.get("signature") else ""
        output.append(f"  [{s['type']}] {s['name']}{sig}")
        output.append(f"    {s['file_path']}:{s['line']}")

    return "\n".join(output)


@mcp.tool()
def find_class(name: str) -> str:
    """
    Найти класс/интерфейс по имени.

    Args:
        name: Имя класса или интерфейса

    Returns:
        Путь к файлу и строка, где объявлен класс
    """
    results = db.search_symbols(name, symbol_type=None, limit=10)
    # Фильтруем только классы и интерфейсы
    results = [r for r in results if r["type"] in ("class", "interface", "object", "enum")]

    if not results:
        return f"Класс/интерфейс '{name}' не найден"

    output = []
    for s in results:
        output.append(f"[{s['type']}] {s['name']}: {s['file_path']}:{s['line']}")

    return "\n".join(output)


@mcp.tool()
def get_file_outline(file_path: str) -> str:
    """
    Получить структуру файла (список классов, функций и т.д.).

    Args:
        file_path: Путь к файлу

    Returns:
        Структура файла с символами
    """
    symbols = db.get_file_symbols(file_path)

    if not symbols:
        return f"Символы в файле '{file_path}' не найдены (возможно файл не проиндексирован)"

    output = [f"Структура файла {file_path}:"]

    for s in symbols:
        indent = "  " if s.get("parent_symbol_id") else ""
        sig = f" {s['signature']}" if s.get("signature") else ""
        output.append(f"{indent}[{s['type']}] {s['name']}{sig} (строка {s['line']})")

    return "\n".join(output)


# === Инструменты работы с модулями ===

@mcp.tool()
def find_module(query: str, limit: int = 20) -> str:
    """
    Поиск модулей по имени.

    Args:
        query: Имя модуля или его часть
        limit: Максимальное количество результатов

    Returns:
        Список найденных модулей
    """
    results = db.search_modules(query, limit)

    if not results:
        return f"Модули по запросу '{query}' не найдены"

    output = [f"Найдено модулей: {len(results)}"]
    for m in results:
        output.append(f"  [{m['type']}] {m['name']}")
        output.append(f"    {m['path']}")

    return "\n".join(output)


@mcp.tool()
def get_module_deps(module_name: str) -> str:
    """
    Получить зависимости модуля.

    Args:
        module_name: Имя модуля (например, features.payments.api)

    Returns:
        Список зависимостей модуля
    """
    deps = db.get_module_deps(module_name)

    if not deps:
        return f"Зависимости модуля '{module_name}' не найдены"

    output = [f"Зависимости модуля {module_name}:"]

    # Группируем по типу
    by_type = {}
    for d in deps:
        dep_type = d["dep_type"]
        if dep_type not in by_type:
            by_type[dep_type] = []
        by_type[dep_type].append(d["dep_module_name"])

    for dep_type, modules in sorted(by_type.items()):
        output.append(f"\n  {dep_type}:")
        for m in sorted(modules):
            output.append(f"    - {m}")

    return "\n".join(output)


@mcp.tool()
def get_module_dependents(module_name: str) -> str:
    """
    Получить модули, которые зависят от данного.

    Args:
        module_name: Имя модуля

    Returns:
        Список модулей-зависимых
    """
    deps = db.get_module_dependents(module_name)

    if not deps:
        return f"От модуля '{module_name}' никто не зависит"

    output = [f"От модуля {module_name} зависят:"]
    for d in deps:
        output.append(f"  [{d['dep_type']}] {d['module_name']}")

    return "\n".join(output)


# === Инструменты индексации ===

@mcp.tool()
def rebuild_index(index_type: str = "all") -> str:
    """
    Пересобрать индекс.

    Args:
        index_type: Тип индекса для пересборки:
            - "files" - только файлы
            - "modules" - только модули
            - "symbols" - только символы
            - "all" - всё (по умолчанию)

    Returns:
        Статистика индексации
    """
    results = {}

    if index_type in ("all", "files"):
        results["files"] = file_indexer.index_all()

    if index_type in ("all", "modules"):
        results["modules"] = module_indexer.index_all()

    if index_type in ("all", "symbols"):
        results["symbols"] = symbol_indexer.index_all()

    # Обновляем метаданные
    db.set_meta("last_indexed", time.strftime("%Y-%m-%d %H:%M:%S"))
    db.commit()

    output = ["Индексация завершена:"]
    for idx_type, stats in results.items():
        output.append(f"\n  {idx_type}:")
        for key, value in stats.items():
            output.append(f"    {key}: {value}")

    return "\n".join(output)


@mcp.tool()
def get_index_stats() -> str:
    """
    Получить статистику индекса.

    Returns:
        Статистика индекса (количество файлов, модулей, символов)
    """
    stats = db.get_stats()

    output = [
        "Статистика индекса:",
        f"  Файлов: {stats['files']}",
        f"  Модулей: {stats['modules']}",
        f"  Символов: {stats['symbols']}",
        f"  Зависимостей: {stats['dependencies']}",
        f"  Последняя индексация: {stats['last_indexed'] or 'никогда'}",
    ]

    return "\n".join(output)


# === Комбинированный поиск ===

@mcp.tool()
def search(query: str, limit: int = 10) -> str:
    """
    Универсальный поиск по файлам, символам и модулям.

    Args:
        query: Строка поиска
        limit: Максимальное количество результатов каждого типа

    Returns:
        Результаты поиска по всем категориям
    """
    output = [f"Результаты поиска '{query}':"]

    # Поиск файлов
    files = db.search_files(query, limit)
    if files:
        output.append(f"\n--- Файлы ({len(files)}) ---")
        for f in files[:5]:
            output.append(f"  {f['path']}")
        if len(files) > 5:
            output.append(f"  ... и ещё {len(files) - 5}")

    # Поиск символов
    symbols = db.search_symbols(query, limit=limit)
    if symbols:
        output.append(f"\n--- Символы ({len(symbols)}) ---")
        for s in symbols[:5]:
            output.append(f"  [{s['type']}] {s['name']} - {s['file_path']}:{s['line']}")
        if len(symbols) > 5:
            output.append(f"  ... и ещё {len(symbols) - 5}")

    # Поиск модулей
    modules = db.search_modules(query, limit)
    if modules:
        output.append(f"\n--- Модули ({len(modules)}) ---")
        for m in modules[:5]:
            output.append(f"  [{m['type']}] {m['name']}")
        if len(modules) > 5:
            output.append(f"  ... и ещё {len(modules) - 5}")

    if len(output) == 1:
        return f"По запросу '{query}' ничего не найдено"

    return "\n".join(output)


if __name__ == "__main__":
    mcp.run()
