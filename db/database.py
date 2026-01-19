import sqlite3
import time
from pathlib import Path
from typing import Optional

from .schema import init_schema


class Database:
    """SQLite база данных для индекса проекта."""

    def __init__(self, db_path: str):
        self.db_path = db_path
        self.conn: Optional[sqlite3.Connection] = None

    def connect(self):
        """Подключение к БД."""
        self.conn = sqlite3.connect(self.db_path, check_same_thread=False)
        self.conn.row_factory = sqlite3.Row
        init_schema(self.conn)

    def close(self):
        """Закрытие соединения."""
        if self.conn:
            self.conn.close()
            self.conn = None

    def execute(self, query: str, params: tuple = ()):
        """Выполнение запроса."""
        return self.conn.execute(query, params)

    def executemany(self, query: str, params_list: list):
        """Выполнение множества запросов."""
        return self.conn.executemany(query, params_list)

    def commit(self):
        """Коммит транзакции."""
        self.conn.commit()

    # === Files ===

    def upsert_file(self, path: str, name: str, extension: str, module: str, modified_at: float):
        """Добавить или обновить файл."""
        self.execute("""
            INSERT INTO files (path, name, extension, module, modified_at, indexed_at)
            VALUES (?, ?, ?, ?, ?, ?)
            ON CONFLICT(path) DO UPDATE SET
                name = excluded.name,
                extension = excluded.extension,
                module = excluded.module,
                modified_at = excluded.modified_at,
                indexed_at = excluded.indexed_at
        """, (path, name, extension, module, modified_at, time.time()))

    def get_file_by_path(self, path: str) -> Optional[dict]:
        """Получить файл по пути."""
        row = self.execute("SELECT * FROM files WHERE path = ?", (path,)).fetchone()
        return dict(row) if row else None

    def search_files(self, query: str, limit: int = 20) -> list[dict]:
        """Поиск файлов по имени."""
        rows = self.execute("""
            SELECT * FROM files
            WHERE name LIKE ? OR path LIKE ?
            ORDER BY
                CASE WHEN name = ? THEN 0
                     WHEN name LIKE ? THEN 1
                     ELSE 2 END,
                length(path)
            LIMIT ?
        """, (f"%{query}%", f"%{query}%", query, f"{query}%", limit)).fetchall()
        return [dict(row) for row in rows]

    def search_files_fts(self, query: str, limit: int = 20) -> list[dict]:
        """Полнотекстовый поиск файлов."""
        rows = self.execute("""
            SELECT f.* FROM files f
            JOIN files_fts fts ON f.path = fts.path
            WHERE files_fts MATCH ?
            LIMIT ?
        """, (query, limit)).fetchall()
        return [dict(row) for row in rows]

    def delete_files_by_module(self, module: str):
        """Удалить файлы модуля."""
        self.execute("DELETE FROM files WHERE module = ?", (module,))

    def get_all_file_paths(self) -> set[str]:
        """Получить все пути файлов."""
        rows = self.execute("SELECT path FROM files").fetchall()
        return {row["path"] for row in rows}

    def delete_files_by_paths(self, paths: list[str]):
        """Удалить файлы по путям."""
        if not paths:
            return
        # SQLite имеет лимит на количество переменных (~999), разбиваем на батчи
        batch_size = 500
        for i in range(0, len(paths), batch_size):
            batch = paths[i:i + batch_size]
            placeholders = ",".join("?" * len(batch))
            self.execute(f"DELETE FROM files WHERE path IN ({placeholders})", tuple(batch))

    def rebuild_files_fts(self):
        """Пересобрать FTS индекс файлов."""
        self.execute("DELETE FROM files_fts")
        self.execute("""
            INSERT INTO files_fts (name, path, module)
            SELECT name, path, module FROM files
        """)

    # === Modules ===

    def upsert_module(self, name: str, path: str, module_type: str):
        """Добавить или обновить модуль."""
        self.execute("""
            INSERT INTO modules (name, path, type)
            VALUES (?, ?, ?)
            ON CONFLICT(name) DO UPDATE SET
                path = excluded.path,
                type = excluded.type
        """, (name, path, module_type))

    def get_module_by_name(self, name: str) -> Optional[dict]:
        """Получить модуль по имени."""
        row = self.execute("SELECT * FROM modules WHERE name = ?", (name,)).fetchone()
        return dict(row) if row else None

    def search_modules(self, query: str, limit: int = 20) -> list[dict]:
        """Поиск модулей по имени."""
        rows = self.execute("""
            SELECT * FROM modules
            WHERE name LIKE ?
            ORDER BY length(name)
            LIMIT ?
        """, (f"%{query}%", limit)).fetchall()
        return [dict(row) for row in rows]

    def get_all_modules(self) -> list[dict]:
        """Получить все модули."""
        rows = self.execute("SELECT * FROM modules ORDER BY name").fetchall()
        return [dict(row) for row in rows]

    def clear_modules(self):
        """Очистить таблицу модулей."""
        self.execute("DELETE FROM modules")
        self.execute("DELETE FROM module_deps")

    # === Module Dependencies ===

    def add_module_dep(self, module_id: int, dep_module_name: str, dep_type: str):
        """Добавить зависимость модуля."""
        self.execute("""
            INSERT OR IGNORE INTO module_deps (module_id, dep_module_name, dep_type)
            VALUES (?, ?, ?)
        """, (module_id, dep_module_name, dep_type))

    def get_module_deps(self, module_name: str) -> list[dict]:
        """Получить зависимости модуля."""
        rows = self.execute("""
            SELECT md.dep_module_name, md.dep_type
            FROM module_deps md
            JOIN modules m ON md.module_id = m.id
            WHERE m.name = ?
            ORDER BY md.dep_type, md.dep_module_name
        """, (module_name,)).fetchall()
        return [dict(row) for row in rows]

    def get_module_dependents(self, module_name: str) -> list[dict]:
        """Получить модули, зависящие от данного."""
        rows = self.execute("""
            SELECT m.name as module_name, md.dep_type
            FROM module_deps md
            JOIN modules m ON md.module_id = m.id
            WHERE md.dep_module_name LIKE ?
            ORDER BY m.name
        """, (f"%{module_name}%",)).fetchall()
        return [dict(row) for row in rows]

    # === Symbols ===

    def upsert_symbol(self, name: str, symbol_type: str, file_id: int, line: int,
                      end_line: int = None, signature: str = None,
                      parent_id: int = None, visibility: str = None) -> int:
        """Добавить или обновить символ."""
        cursor = self.execute("""
            INSERT INTO symbols (name, type, file_id, line, end_line, signature, parent_symbol_id, visibility)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        """, (name, symbol_type, file_id, line, end_line, signature, parent_id, visibility))
        return cursor.lastrowid

    def delete_symbols_by_file(self, file_id: int):
        """Удалить символы файла."""
        self.execute("DELETE FROM symbols WHERE file_id = ?", (file_id,))

    def search_symbols(self, query: str, symbol_type: str = None, limit: int = 20) -> list[dict]:
        """Поиск символов по имени."""
        if symbol_type:
            rows = self.execute("""
                SELECT s.*, f.path as file_path, f.module
                FROM symbols s
                JOIN files f ON s.file_id = f.id
                WHERE s.name LIKE ? AND s.type = ?
                ORDER BY
                    CASE WHEN s.name = ? THEN 0
                         WHEN s.name LIKE ? THEN 1
                         ELSE 2 END,
                    length(s.name)
                LIMIT ?
            """, (f"%{query}%", symbol_type, query, f"{query}%", limit)).fetchall()
        else:
            rows = self.execute("""
                SELECT s.*, f.path as file_path, f.module
                FROM symbols s
                JOIN files f ON s.file_id = f.id
                WHERE s.name LIKE ?
                ORDER BY
                    CASE WHEN s.name = ? THEN 0
                         WHEN s.name LIKE ? THEN 1
                         ELSE 2 END,
                    length(s.name)
                LIMIT ?
            """, (f"%{query}%", query, f"{query}%", limit)).fetchall()
        return [dict(row) for row in rows]

    def get_file_symbols(self, file_path: str) -> list[dict]:
        """Получить символы файла."""
        rows = self.execute("""
            SELECT s.* FROM symbols s
            JOIN files f ON s.file_id = f.id
            WHERE f.path = ?
            ORDER BY s.line
        """, (file_path,)).fetchall()
        return [dict(row) for row in rows]

    def rebuild_symbols_fts(self):
        """Пересобрать FTS индекс символов."""
        self.execute("DELETE FROM symbols_fts")
        self.execute("""
            INSERT INTO symbols_fts (name, signature)
            SELECT name, COALESCE(signature, '') FROM symbols
        """)

    # === Meta ===

    def set_meta(self, key: str, value: str):
        """Установить метаданные."""
        self.execute("""
            INSERT INTO index_meta (key, value)
            VALUES (?, ?)
            ON CONFLICT(key) DO UPDATE SET value = excluded.value
        """, (key, value))

    def get_meta(self, key: str) -> Optional[str]:
        """Получить метаданные."""
        row = self.execute("SELECT value FROM index_meta WHERE key = ?", (key,)).fetchone()
        return row["value"] if row else None

    # === Stats ===

    def get_stats(self) -> dict:
        """Получить статистику индекса."""
        files_count = self.execute("SELECT COUNT(*) as cnt FROM files").fetchone()["cnt"]
        modules_count = self.execute("SELECT COUNT(*) as cnt FROM modules").fetchone()["cnt"]
        symbols_count = self.execute("SELECT COUNT(*) as cnt FROM symbols").fetchone()["cnt"]
        deps_count = self.execute("SELECT COUNT(*) as cnt FROM module_deps").fetchone()["cnt"]

        last_indexed = self.get_meta("last_indexed")

        return {
            "files": files_count,
            "modules": modules_count,
            "symbols": symbols_count,
            "dependencies": deps_count,
            "last_indexed": last_indexed,
        }
