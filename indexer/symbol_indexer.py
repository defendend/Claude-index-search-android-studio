import re
import sys
import time
from pathlib import Path
from typing import Optional

# Добавляем родительскую директорию в путь
sys.path.insert(0, str(Path(__file__).parent.parent))

from db.database import Database

# Попробуем импортировать tree-sitter, если доступен
try:
    import tree_sitter_kotlin as ts_kotlin
    from tree_sitter import Language, Parser
    TREE_SITTER_AVAILABLE = True
except ImportError:
    TREE_SITTER_AVAILABLE = False


class SymbolIndexer:
    """Индексатор символов (классы, функции, интерфейсы)."""

    # Regex паттерны для fallback парсинга
    CLASS_PATTERN = re.compile(
        r'^(?:(?:public|private|internal|protected|abstract|open|sealed|data|enum|annotation)\s+)*'
        r'(class|interface|object|enum\s+class)\s+(\w+)',
        re.MULTILINE
    )

    FUNCTION_PATTERN = re.compile(
        r'^(?:\s*)(?:(?:public|private|internal|protected|override|suspend|inline|operator)\s+)*'
        r'fun\s+(?:<[^>]+>\s+)?(\w+)\s*\(([^)]*)\)',
        re.MULTILINE
    )

    PROPERTY_PATTERN = re.compile(
        r'^(?:\s*)(?:(?:public|private|internal|protected|override|lateinit|const)\s+)*'
        r'(val|var)\s+(\w+)\s*[:=]',
        re.MULTILINE
    )

    def __init__(self, db: Database, project_root: str):
        self.db = db
        self.project_root = Path(project_root)
        self.parser = None

        if TREE_SITTER_AVAILABLE:
            self._init_tree_sitter()

    def _init_tree_sitter(self):
        """Инициализация tree-sitter парсера."""
        try:
            self.parser = Parser(Language(ts_kotlin.language()))
        except Exception as e:
            print(f"Failed to init tree-sitter: {e}", file=sys.stderr)
            self.parser = None

    def index_all(self, progress_callback: Optional[callable] = None) -> dict:
        """Полная индексация символов во всех Kotlin файлах."""
        start_time = time.time()
        stats = {"files": 0, "symbols": 0, "errors": 0}

        # Получаем все Kotlin файлы из индекса
        kt_files = self.db.execute(
            "SELECT id, path FROM files WHERE extension = '.kt'"
        ).fetchall()

        for file_row in kt_files:
            file_id = file_row["id"]
            file_path = file_row["path"]

            try:
                # Удаляем старые символы файла
                self.db.delete_symbols_by_file(file_id)

                # Индексируем новые
                symbols_count = self._index_file(file_id, file_path)
                stats["symbols"] += symbols_count
                stats["files"] += 1

                if progress_callback and stats["files"] % 500 == 0:
                    progress_callback(stats["files"])

            except Exception as e:
                stats["errors"] += 1

        # Пересобираем FTS индекс
        self.db.rebuild_symbols_fts()
        self.db.commit()

        stats["elapsed_seconds"] = round(time.time() - start_time, 2)
        return stats

    def index_file(self, file_path: str) -> int:
        """Индексация символов одного файла."""
        file_info = self.db.get_file_by_path(file_path)
        if not file_info:
            return 0

        self.db.delete_symbols_by_file(file_info["id"])
        count = self._index_file(file_info["id"], file_path)
        self.db.commit()
        return count

    def _index_file(self, file_id: int, file_path: str) -> int:
        """Внутренняя индексация файла."""
        try:
            content = Path(file_path).read_text(encoding="utf-8")
        except Exception:
            return 0

        if self.parser:
            return self._index_with_tree_sitter(file_id, content)
        else:
            return self._index_with_regex(file_id, content)

    def _index_with_tree_sitter(self, file_id: int, content: str) -> int:
        """Индексация с использованием tree-sitter."""
        tree = self.parser.parse(bytes(content, "utf-8"))
        symbols_count = 0

        def visit_node(node, parent_id=None):
            nonlocal symbols_count

            symbol_type = None
            name = None
            signature = None

            if node.type == "class_declaration":
                # Определяем тип: class, interface или enum
                symbol_type = self._detect_class_type(node)
                name = self._get_identifier(node, content)
            elif node.type == "object_declaration":
                symbol_type = "object"
                name = self._get_identifier(node, content)
            elif node.type == "function_declaration":
                symbol_type = "function"
                name = self._get_identifier(node, content)
                # Получаем параметры функции
                for child in node.children:
                    if child.type == "function_value_parameters":
                        signature = content[child.start_byte:child.end_byte]
                        break
            elif node.type == "property_declaration":
                symbol_type = "property"
                # Для property имя в variable_declaration -> identifier
                name = self._get_property_name(node, content)
                # Получаем тип свойства
                signature = self._get_property_type(node, content)

            current_parent_id = parent_id

            if symbol_type and name:
                line = node.start_point[0] + 1
                end_line = node.end_point[0] + 1

                symbol_id = self.db.upsert_symbol(
                    name=name,
                    symbol_type=symbol_type,
                    file_id=file_id,
                    line=line,
                    end_line=end_line,
                    signature=signature,
                    parent_id=parent_id,
                )
                symbols_count += 1
                current_parent_id = symbol_id

            for child in node.children:
                visit_node(child, current_parent_id)

        visit_node(tree.root_node)
        return symbols_count

    def _detect_class_type(self, node) -> str:
        """Определить тип class_declaration: class, interface или enum."""
        for child in node.children:
            if child.type == "interface":
                return "interface"
            elif child.type == "modifiers":
                for mod in child.children:
                    if mod.type == "class_modifier" and mod.children:
                        for m in mod.children:
                            if m.type == "enum":
                                return "enum"
        return "class"

    def _get_identifier(self, node, content: str) -> Optional[str]:
        """Получить identifier напрямую из детей узла."""
        for child in node.children:
            if child.type == "identifier":
                return content[child.start_byte:child.end_byte]
        return None

    def _get_property_name(self, node, content: str) -> Optional[str]:
        """Получить имя свойства из property_declaration."""
        for child in node.children:
            if child.type == "variable_declaration":
                # Ищем identifier внутри variable_declaration
                for subchild in child.children:
                    if subchild.type == "identifier":
                        return content[subchild.start_byte:subchild.end_byte]
        return None

    def _get_property_type(self, node, content: str) -> Optional[str]:
        """Получить тип свойства."""
        for child in node.children:
            if child.type == "variable_declaration":
                for subchild in child.children:
                    if subchild.type == "user_type":
                        return content[subchild.start_byte:subchild.end_byte]
        return None

    def _get_child_text(self, node, child_type: str, content: str) -> Optional[str]:
        """Получить текст дочернего узла."""
        for child in node.children:
            if child.type == child_type:
                return content[child.start_byte:child.end_byte]
            # Рекурсивно ищем в children
            result = self._get_child_text(child, child_type, content)
            if result:
                return result
        return None

    def _index_with_regex(self, file_id: int, content: str) -> int:
        """Fallback индексация с regex."""
        symbols_count = 0
        lines = content.split("\n")

        # Находим классы/интерфейсы/объекты
        for match in self.CLASS_PATTERN.finditer(content):
            symbol_type_raw = match.group(1)
            name = match.group(2)

            if "interface" in symbol_type_raw:
                symbol_type = "interface"
            elif "object" in symbol_type_raw:
                symbol_type = "object"
            elif "enum" in symbol_type_raw:
                symbol_type = "enum"
            else:
                symbol_type = "class"

            line = content[:match.start()].count("\n") + 1

            self.db.upsert_symbol(
                name=name,
                symbol_type=symbol_type,
                file_id=file_id,
                line=line,
            )
            symbols_count += 1

        # Находим top-level функции
        for match in self.FUNCTION_PATTERN.finditer(content):
            name = match.group(1)
            params = match.group(2)
            line = content[:match.start()].count("\n") + 1

            # Проверяем, что это не внутри класса (простая эвристика)
            line_start = content.rfind("\n", 0, match.start()) + 1
            indent = len(content[line_start:match.start()]) - len(content[line_start:match.start()].lstrip())

            if indent == 0:  # Top-level функция
                self.db.upsert_symbol(
                    name=name,
                    symbol_type="function",
                    file_id=file_id,
                    line=line,
                    signature=f"({params})",
                )
                symbols_count += 1

        return symbols_count
