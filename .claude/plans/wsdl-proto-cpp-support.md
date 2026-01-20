# План: Поддержка WSDL/Proto/C++ в ast-index

## Контекст

В `arcadia/direct` обнаружены:
- **Proto**: `direct/intapi/proto/` (proto2), `direct/api/v6/` (proto3)
- **WSDL/XSD**: `direct/perl/api/wsdl/v5/` (31 WSDL + 4 XSD файлов) - **ШАБЛОНЫ Template Toolkit!**
- **C++**: `direct/libs/*/jni/` (JNI биндинги, современный C++11/14)

## Анализ сложности по языкам

### 1. Protocol Buffers (.proto) - **Низкая сложность** ⭐

**Реальные примеры из arcadia/direct:**

**proto2** (`intapi/proto/agency_change_request.proto`):
```protobuf
package NDirect.ChangeAgency;
import "grut/proto/api/objects/objects.proto";
option java_package = "ru.yandex.direct.intapi.entity.agency.model";

message TChangeAgencyRequest {
    message TChangeAgencyRequestItem {
        optional uint64 client_id = 1;
        optional uint64 new_agency_client_id = 2;
    }
    repeated TChangeAgencyRequestItem items = 1;
}
```

**proto3** (`api/v6/services/campaign_service.proto`):
```protobuf
syntax = "proto3";
package direct.api.v6.services;
import "direct/api/v6/resources/campaign.proto";
import "google/api/annotations.proto";

service CampaignService {
    rpc GetCampaign(GetCampaignRequest) returns (Campaign) {
        option (google.api.http) = { get: "/v6/campaigns/{campaign_id}" };
    }
    rpc ListCampaigns(ListCampaignsRequest) returns (ListCampaignsResponse);
}

message GetCampaignRequest {
    int64 campaign_id = 2 [(google.api.field_behavior) = REQUIRED];
}
```

**Что индексировать:**
- `message` - как class (включая вложенные)
- `service` - как interface
- `rpc` - как method
- `enum` - как enum
- `import` - как import
- `package` - как module
- `option java_package` - для cross-reference с Java

**Особенности Direct:**
- Используются **оба** proto2 и proto3
- Вложенные message (`TChangeAgencyRequestItem` внутри `TChangeAgencyRequest`)
- Google API annotations (`google.api.http`, `google.api.field_behavior`)
- Yandex-специфичные опции (`java_package`, `java_outer_classname`)

**Оценка:** 3-4 часа работы (чуть больше из-за поддержки обоих версий)

---

### 2. WSDL/XSD (.wsdl, .xsd) - **Средняя сложность** ⭐⭐

**ВАЖНО:** WSDL файлы в Direct - это **Template Toolkit шаблоны**, не чистый XML!

**Реальный пример** (`direct/perl/api/wsdl/v5/Campaigns.wsdl`):
```xml
<?xml version="1.0" encoding="UTF-8"?>
<wsdl:definitions xmlns:ns="http://api.direct.yandex.com/v5/campaigns"
                  name="Campaigns" targetNamespace="http://api.direct.yandex.com/v5/campaigns">
    <wsdl:types>
        <xsd:schema>
           <xsd:import namespace="http://api.direct.yandex.com/v5/general"
                       schemaLocation="[% API_SERVER_PATH %]/v[% api_version %]/general.xsd" />
[% strategy_settings_required_for_add = { ... } %]
[%- BLOCK strategy_type %]
            <xsd:complexType name="Strategy[% name %][% method %]">
                [% content %]
            </xsd:complexType>
[%- END -%]
```

**XSD файлы чистые** (`general.xsd`):
```xml
<xsd:schema xmlns:xsd="http://www.w3.org/2001/XMLSchema">
    <xsd:complexType name="ArrayOfString">
        <xsd:sequence>
            <xsd:element name="Items" type="xsd:string" minOccurs="1" maxOccurs="unbounded"/>
        </xsd:sequence>
    </xsd:complexType>
    <xsd:simpleType name="CurrencyEnum">
        <xsd:restriction base="xsd:string">
            <xsd:enumeration value="RUB"/>
            <xsd:enumeration value="USD"/>
        </xsd:restriction>
    </xsd:simpleType>
</xsd:schema>
```

**Стратегия парсинга:**
1. **XSD файлы** - парсить полностью (чистый XML)
2. **WSDL шаблоны** - игнорировать `[% ... %]` блоки, парсить XML части

**Что индексировать:**
- `<xsd:complexType name="...">` - как class
- `<xsd:simpleType name="...">` - как enum (если restriction с enumeration)
- `<xsd:element name="...">` - как property
- `<wsdl:service name="...">` - как class
- `<wsdl:portType name="...">` - как interface
- `<wsdl:operation name="...">` - как method
- `<xsd:import>` / `<xsd:include>` - как import

**Варианты реализации:**
1. **Regex + strip TT** - убрать `[%...%]`, парсить regex (быстро)
2. **quick-xml после strip** - надёжнее для сложных случаев

**Файлы в Direct:**
- 31 WSDL файлов (шаблоны)
- 4 XSD файлов (чистые)

**Оценка:** 5-7 часов (из-за Template Toolkit)

---

### 3. C++ (.cpp, .h, .hpp, .cc) - **Средняя сложность** ⭐⭐⭐

**ХОРОШАЯ НОВОСТЬ:** C++ в Direct - это в основном **JNI биндинги**, относительно простой код!

**Реальный пример** (`direct/libs/textprocessing/jni/util.h`):
```cpp
#pragma once
#include <jni.h>
#include <memory>
#include <util/generic/string.h>

class TJavaException {
public:
    TJavaException() {}
    TJavaException(JNIEnv* env, const char* classname, const char* message);
};

template<class Func>
inline auto jniWrapExceptions(JNIEnv* env, Func&& func) {
    try { return func(); }
    catch (const std::exception& e) { ... }
}

template<class T>
class TJniReference : public TNonCopyable {
    JNIEnv* env_;
    T value_;
public:
    TJniReference(JNIEnv* env, T value);
    T Get() const;
    void Reset();
};

class TJniClass : public TJniReference<jclass> {
public:
    TJniClass(JNIEnv* env, const char* name);
    jmethodID GetMethodID(const char* name, const char* sig);
};
```

**Реальный пример** (`TextProcessing.cpp`):
```cpp
#include "ru_yandex_direct_textprocessing_TextProcessing.h"
#include <ads/clemmer/lib/clemmer.h>
#include <library/cpp/text_processing/tokenizer/tokenizer.h>

class TClemmerResult {
    clemmer2_result result_;
public:
    TClemmerResult(clemmer2_result result) : result_(result) { }
    ~TClemmerResult() { Reset(); }
    clemmer2_result Get() const { return result_; }
};

JNIEXPORT jobject JNICALL Java_ru_yandex_direct_textprocessing_TextProcessing_analyze2(
    JNIEnv* env, jclass, jstring phrase, jint lang, jboolean fillformas) {
    return jniWrapExceptions(env, [&]() -> jobject { ... });
}
```

**Особенности Direct C++:**
- JNI биндинги (JNIEXPORT, JNIEnv*, jclass, jstring)
- Yandex типы (TString, TVector, TNonCopyable)
- Простые templates (в основном RAII wrappers)
- Современный C++11/14 (lambdas, auto, move semantics)
- **НЕТ** тяжёлых boost-стиль templates
- **НЕТ** сложного множественного наследования

**Что индексировать:**
- `class` / `struct` - как class
- методы (включая JNIEXPORT)
- `#include` - как import
- `template<class T>` - простые случаи
- наследование (`: public Base`)

**Файлы в Direct:**
- 5 .cpp файлов
- 5 .h файлов
- Все в `libs/*/jni/` директориях

**Оценка:** 6-8 часов (проще чем ожидалось из-за ограниченного scope)

---

## Рекомендуемый план реализации

### Фаза 1: Proto (быстрая победа)
```
Время: 2-3 часа
Файлы:
  - src/parsers/proto.rs (новый)
  - src/parsers/mod.rs (добавить proto)
  - src/commands/proto.rs (новый, опционально)
  - skills/ast-index/references/proto-commands.md
```

**Команды:**
- `ast-index search` - будет находить proto
- `ast-index class "UserRequest"` - найдёт message
- `ast-index usages "UserService"` - найдёт использования

### Фаза 2: WSDL
```
Время: 4-6 часов
Файлы:
  - src/parsers/wsdl.rs (новый)
  - src/parsers/mod.rs (добавить wsdl)
  - skills/ast-index/references/wsdl-commands.md
Зависимости:
  - quick-xml = "0.31" (опционально, можно regex)
```

### Фаза 3: C++ (базовая поддержка)
```
Время: 8-12 часов
Файлы:
  - src/parsers/cpp.rs (новый)
  - src/parsers/mod.rs (добавить cpp)
  - src/commands/cpp.rs (новый, для специфичных команд)
  - skills/ast-index/references/cpp-commands.md
```

**Базовые возможности:**
- class/struct definitions
- function definitions
- #include parsing
- namespace detection
- basic inheritance

**Отложить на потом:**
- templates полный парсинг
- препроцессор макросы
- operator overloading

---

## Структура парсеров (текущая vs новая)

### Текущая:
```
src/parsers/
├── mod.rs
├── kotlin.rs    # Kotlin + Java
├── swift.rs     # Swift + ObjC
└── perl.rs      # Perl
```

### После расширения:
```
src/parsers/
├── mod.rs
├── kotlin.rs
├── swift.rs
├── perl.rs
├── proto.rs     # NEW: Protocol Buffers
├── wsdl.rs      # NEW: WSDL/XSD
└── cpp.rs       # NEW: C/C++
```

---

## Определение типа проекта

Добавить в auto-detection:

| Маркер файла | Тип проекта |
|--------------|-------------|
| `*.proto` в корне | Proto project |
| `*.wsdl` в корне | WSDL project |
| `CMakeLists.txt` | C++ (CMake) |
| `Makefile` + `*.cpp` | C++ (Make) |
| `*.vcxproj` | C++ (Visual Studio) |
| `BUILD` + `*.cc` | C++ (Bazel) |

---

## Оценка общего времени (обновлённая после анализа)

| Компонент | Время | Приоритет | Обоснование |
|-----------|-------|-----------|-------------|
| Proto parser | 3-4ч | P0 | proto2 + proto3, простой regex |
| Proto skill docs | 1ч | P0 | |
| WSDL/XSD parser | 5-7ч | P1 | Template Toolkit требует strip |
| WSDL skill docs | 1ч | P1 | |
| C++ parser | 6-8ч | P2 | JNI биндинги, простой scope |
| C++ skill docs | 1ч | P2 | |
| **Итого** | **17-22ч** | | |

---

## Ответы на вопросы (из анализа кода)

1. **Proto**: Используются **ОБА** - proto2 (`intapi/proto/`) и proto3 (`api/v6/`)
2. **WSDL**: XSD файлы **отдельно** (4 шт), WSDL - **Template Toolkit шаблоны** (31 шт)
3. **C++**:
   - Стандарт: **C++11/14** (lambdas, auto, move)
   - Templates: **Простые** (RAII wrappers, без boost-стиля)
   - Bazel: **Не нужно** (только ya.make)
4. **Общее**: Базовых команд (search/class/usages) **достаточно**

---

## Следующие шаги

1. [x] ~~Получить примеры файлов из arcadia/direct для каждого типа~~ ✅ Сделано
2. [x] **Фаза 1: Proto** ✅ Сделано
   - ✅ Создан `src/parsers/proto.rs` (proto2 + proto3)
   - ✅ Добавлен `references/proto-commands.md`
   - ✅ Протестировано на `direct/intapi/proto/` (11 файлов, 46 символов)
3. [x] **Фаза 2: WSDL/XSD** ✅ Сделано
   - ✅ Создан `src/parsers/wsdl.rs`
   - ✅ Template Toolkit стрипинг (`[% ... %]`, `[%- BLOCK -%]`)
   - ✅ Парсинг XSD типов и WSDL сервисов
   - ✅ Протестировано на `direct/perl/api/wsdl/v5/` (35 файлов, 1515 символов)
   - ✅ Добавлен `references/wsdl-commands.md`
4. [x] **Фаза 3: C++** ✅ Сделано
   - ✅ Создан `src/parsers/cpp.rs`
   - ✅ JNI функции (`JNIEXPORT ... JNICALL Java_...`)
   - ✅ Классы, template классы, namespaces, enums
   - ✅ Протестировано на `direct/libs/*/jni/` (4 файла, 23 символа)
   - ✅ Добавлен `references/cpp-commands.md`

## Финальное тестирование

✅ Полный тест на `arcadia/direct`:
- **46027 файлов** проиндексировано
- **380533 символов** извлечено
- Время индексации: ~4.5 минуты

✅ Все парсеры работают:
- Proto: `TChangeAgencyRequest` найден в `intapi/proto/`
- WSDL: `ClientFieldEnum` найден в `perl/api/wsdl/v5/`
- C++: `TJniReference`, `analyze2` найдены в `libs/textprocessing/jni/`

## Версия 3.7.0

Обновлены:
- `Cargo.toml` - версия 3.7.0
- `.claude-plugin/marketplace.json` - версия 3.7.0, описание
- `skills/ast-index/SKILL.md` - таблица платформ, ссылки на reference docs
