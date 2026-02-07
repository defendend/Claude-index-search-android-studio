# Knowledge Compression Engine — Master Plan

> Максимум релевантной информации в минимум токенов

**Дата:** 2026-02-05
**Статус:** Research Complete, Ready for MVP

---

## Executive Summary

Исследование 6 направлений показало: **можно достичь 60-90% сокращения токенов** при сохранении 85-95% качества ответов. Ключ — **гибридная архитектура**, комбинирующая несколько подходов.

### Ключевые метрики из исследований

| Подход | Сжатие токенов | Качество | Стоимость |
|--------|---------------|----------|-----------|
| **Extractive summarization** | 70-85% | F1 0.70-0.85 | $0.001/doc (Haiku) |
| **Hierarchical index** | 85-99% | Recall@10 >70% | Низкая (офлайн) |
| **Structured facts** | 10-15x | F1 0.85-0.90 | $0.10/doc (hybrid) |
| **Query-aware compression** | 60-94% | +15-30% quality | +50-100ms latency |
| **Knowledge graph** | 60x (!) | 80% vs 50% RAG | Medium (graph construction) |

---

## Архитектура: Hybrid Multi-Layer System

```
┌─────────────────────────────────────────────────────────────────────┐
│                         QUERY ROUTER                                 │
│  "What type of query is this?" → Route to optimal layer             │
└─────────────────────────────────────────────────────────────────────┘
                                    │
         ┌──────────────────────────┼──────────────────────────┐
         │                          │                          │
         ▼                          ▼                          ▼
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│   LAYER 1       │     │   LAYER 2       │     │   LAYER 3       │
│  Hierarchical   │     │  Knowledge      │     │  Compressed     │
│  Index          │     │  Graph          │     │  Documents      │
│                 │     │                 │     │                 │
│ • Exact lookup  │     │ • Relationships │     │ • Semantic      │
│ • Navigation    │     │ • Multi-hop     │     │ • Full context  │
│ • O(log n)      │     │ • 60x reduction │     │ • LLMLingua     │
└─────────────────┘     └─────────────────┘     └─────────────────┘
         │                          │                          │
         └──────────────────────────┼──────────────────────────┘
                                    ▼
┌─────────────────────────────────────────────────────────────────────┐
│                      CONTEXT ASSEMBLER                               │
│  Strategic ordering (important at start/end) + Token budget          │
└─────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────┐
│                           LLM                                        │
│  Minimal, relevant, structured context → High quality answer         │
└─────────────────────────────────────────────────────────────────────┘
```

---

## Layer 1: Hierarchical Index (Пирамида знаний)

**Источник:** Research agent #2

### Структура (3-4 уровня оптимально)

```
Level 0: Global Overview        ~2-4K токенов на всю базу
    ↓
Level 1: Cluster Summaries      ~500-1K токенов на кластер
    ↓
Level 2: Document Summaries     ~256-500 токенов на документ
    ↓
Level 3: Detailed Chunks        ~100-1K токенов (только по запросу)
```

### Алгоритм навигации

```python
def navigate_hierarchy(query):
    # 1. Classify query complexity
    if is_global_question(query):
        return fetch_level_0()

    # 2. Top-down traversal
    relevant_clusters = search_level_1(query, top_k=5)
    relevant_docs = search_level_2(query, clusters=relevant_clusters, top_k=20)

    # 3. Fetch details only if needed
    if needs_details(query):
        chunks = search_level_3(query, docs=relevant_docs, top_k=10)
        return combine(relevant_clusters, relevant_docs, chunks)

    return combine(relevant_clusters, relevant_docs)
```

### Результаты
- **85-99% сокращение токенов** vs flat retrieval
- **Recall@10 >74%** (RAPTOR benchmark)
- Алгоритм кластеризации: **Leiden** (для графов) или **HDBSCAN** (для векторов)

---

## Layer 2: Knowledge Graph (Граф сущностей)

**Источник:** Research agent #5

### Схема (Property Graph)

```yaml
# Entities (nodes)
entities:
  - type: Service | Team | Database | Incident | Experiment | Document
    attributes:
      name: string
      owner: string (optional)
      status: string (optional)
      created_at: timestamp
      metadata: JSON

# Relations (edges)
relations:
  - type: DEPENDS_ON | OWNS | CALLS | DOCUMENTS | CAUSED_BY
    attributes:
      weight: float (optional)
      created_at: timestamp
```

### Пример запроса

```
Query: "Что мы знаем про Service X?"

Document RAG: 3 docs × 2000 tokens = 6000 tokens (3.3% relevant)

Graph RAG:
  Service X (owner: team-A, status: healthy)    20 tokens
    → depends_on: [Postgres, Redis]             30 tokens
    → called_by: [API Gateway]                  15 tokens
    → has_incident: [INC-42 (resolved)]         35 tokens
  ─────────────────────────────────────────────
  Total: 100 tokens (100% relevant)

Improvement: 60x reduction, 30x relevance increase
```

### Результаты
- **80% accuracy** vs 50.83% для традиционного RAG
- **60x сокращение токенов**
- **20% лучше** на multi-hop reasoning

### Incremental Updates
- **IncRML approach**: Hash-based change detection
- **315x меньше storage**, 4.6x меньше CPU, 4.4x быстрее

---

## Layer 3: Compressed Documents

**Источник:** Research agents #1, #3, #4

### Pipeline

```
Documents → Retrieval → Reranking → Compression → Strategic Ordering → LLM
```

### Компоненты

#### 3.1 Retrieval (Hybrid)
```
BM25 (keyword) ─┬─→ Reciprocal Rank Fusion → Top 200
Dense (semantic)─┘
```

#### 3.2 Reranking
```
Top 200 → Cross-encoder → Top 20
          (или Cohere Rerank API)
```

#### 3.3 Compression (выбор по use case)

| Метод | Сжатие | Latency | Best for |
|-------|--------|---------|----------|
| **LLMLingua-2** | до 20x | +50-100ms | Aggressive cost reduction |
| **Extractive (RECOMP)** | 2-10x | +10-30ms | Low latency |
| **Selective Context** | 3-8x | +20-50ms | Balanced |

#### 3.4 Strategic Ordering (решает "Lost in the Middle")
```
[High relevance] → [Lower relevance] → [High relevance]
    (начало)           (середина)          (конец)
```

### Результаты
- **60-94% token savings**
- **+15-30% quality improvement** vs no compression
- **Haiku filtering**: 100% recall (не пропускает), но variable precision

---

## Structured Facts Extraction

**Источник:** Research agent #4

### Когда использовать
- Документы с чёткой структурой (runbooks, incidents, experiments)
- Нужен **10-15x compression** с сохранением фактов
- Важна машиночитаемость

### Hybrid Pipeline (Best ROI)

```
Stage 1: spaCy (fast, free)     → 80% documents
Stage 2: LLM (Haiku) selective  → 20% complex cases

Result: 85-90% accuracy at 90% cost reduction vs pure LLM
```

### Пример трансформации

**Input (500 tokens):**
```
Payment Service handles all transactions for the platform. It was
originally built by the Payments team in Q3 2024...
```

**Output (45 tokens):**
```yaml
entities:
  - type: service
    name: Payment Service
    owner: Payments team
    created: 2024-Q3
relationships:
  - type: DEPENDS_ON
    from: Payment Service
    to: [PostgreSQL, Redis]
incidents:
  - date: 2025-01-15
    severity: high
    resolution: "increased pool 20→50"
```

**Compression: 11x**

---

## Query Router Logic

```python
def route_query(query: str) -> QueryStrategy:
    """Route query to optimal layer based on intent"""

    intent = classify_intent(query)

    match intent:
        case "exact_lookup":
            # "Find class UserRepository"
            return Strategy.HIERARCHICAL_INDEX

        case "relationships":
            # "What depends on Service X?"
            return Strategy.KNOWLEDGE_GRAPH

        case "global_summary":
            # "Summarize all services owned by Team A"
            return Strategy.GRAPH_COMMUNITIES

        case "semantic_search":
            # "How do we handle authentication?"
            return Strategy.COMPRESSED_DOCUMENTS

        case "multi_hop":
            # "How does A connect to D through B and C?"
            return Strategy.KNOWLEDGE_GRAPH

        case "unknown":
            return Strategy.HYBRID_ALL_LAYERS
```

---

## Competitive Positioning

**Источник:** Research agent #6

### Market Gaps

| Gap | Opportunity | Our Solution |
|-----|-------------|--------------|
| **Mid-market pricing** | Glean $100K+, Notion weak AI | $10-30/user/month |
| **Code-specific RAG** | General RAG breaks structure | AST-aware + Graph |
| **Production compression** | Research not production-ready | Plug-and-play library |

### Differentiation

1. **Structure-aware**: AST для кода, Graph для relationships, Hierarchy для docs
2. **Guaranteed cost reduction**: 5-10x cheaper than standard vector RAG
3. **No ML expertise required**: Auto-optimization
4. **Developer-first**: CLI/API, не enterprise sales

---

## Implementation Roadmap

### Phase 1: MVP (4-6 weeks)

**Goal:** Prove 60%+ token reduction on real data

**Deliverables:**
1. [ ] Hierarchical index (3 levels) для markdown/wiki
2. [ ] Basic knowledge graph (entities + relations) в SQLite
3. [ ] Query router (rule-based)
4. [ ] Integration с Claude Code через ast-index plugin

**Tech Stack:**
- Rust (core, like ast-index)
- SQLite (storage — portable, no deps)
- sentence-transformers (embeddings, optional)

**Success Metrics:**
- 60%+ token reduction
- <100ms query latency
- F1 >0.80 on test dataset

### Phase 2: Production (8-12 weeks)

**Goal:** Robust system with compression

**Deliverables:**
1. [ ] LLMLingua-2 integration
2. [ ] Hybrid retrieval (BM25 + dense)
3. [ ] Cross-encoder reranking
4. [ ] Incremental updates (hash-based)
5. [ ] RAGAS evaluation pipeline

**Success Metrics:**
- 80%+ token reduction
- Faithfulness >0.85
- <200ms end-to-end latency

### Phase 3: Scale (12-20 weeks)

**Goal:** Multi-source, enterprise-ready

**Deliverables:**
1. [ ] Confluence/Notion/Slack connectors
2. [ ] Structured facts extraction pipeline
3. [ ] GraphRAG communities
4. [ ] Cost monitoring dashboard
5. [ ] Multi-tenant support

**Success Metrics:**
- 90%+ token reduction
- Support for 1M+ documents
- $0.001/query cost

---

## Technical Decisions

### Storage: SQLite

**Why:**
- Zero dependencies (like ast-index)
- Single file, easy backup/sync
- FTS5 for full-text search
- JSON1 for flexible metadata
- Sufficient for <1M entities

**Schema:**
```sql
-- Hierarchical index
CREATE TABLE hierarchy (
    id INTEGER PRIMARY KEY,
    level INTEGER NOT NULL,  -- 0=global, 1=cluster, 2=doc, 3=chunk
    parent_id INTEGER,
    content TEXT,
    summary TEXT,
    token_count INTEGER,
    embedding BLOB
);

-- Knowledge graph
CREATE TABLE entities (
    id INTEGER PRIMARY KEY,
    type TEXT NOT NULL,
    name TEXT NOT NULL,
    attributes JSON,
    source_file TEXT
);

CREATE TABLE relations (
    source_id INTEGER,
    target_id INTEGER,
    type TEXT NOT NULL,
    attributes JSON
);

-- Full-text search
CREATE VIRTUAL TABLE hierarchy_fts USING fts5(content, summary);
CREATE VIRTUAL TABLE entities_fts USING fts5(name, attributes);
```

### Embeddings: Optional

**Start without embeddings** (keyword search + structure)
**Add later** for semantic search if needed

If needed:
- sentence-transformers/all-MiniLM-L6-v2 (384 dims, fast)
- Or OpenAI text-embedding-3-small (1536 dims, better quality)

### Compression: LLMLingua-2

**Why:**
- Up to 20x compression
- 3-6x faster than LLMLingua-1
- Research-backed, open source
- Python library (can call from Rust via subprocess)

---

## Risk Mitigation

| Risk | Mitigation |
|------|------------|
| **Over-compression loses critical info** | Track Faithfulness metric, set minimum thresholds |
| **Graph construction too slow** | Incremental updates, background processing |
| **Complexity overhead** | Start simple (hierarchy only), add layers incrementally |
| **Embeddings expensive** | Make optional, use cheap models (MiniLM) |

---

## Success Criteria

### MVP (Phase 1)
- [ ] 60%+ token reduction on test corpus
- [ ] Query latency <100ms
- [ ] Integration with ast-index working

### Production (Phase 2)
- [ ] 80%+ token reduction
- [ ] Faithfulness >0.85
- [ ] 1000+ documents supported
- [ ] Incremental updates working

### Scale (Phase 3)
- [ ] 90%+ token reduction
- [ ] 100K+ documents
- [ ] Multi-source connectors
- [ ] Cost <$0.01/query

---

## Next Steps

1. **Immediate:** Create test dataset (wiki pages, runbooks, experiments)
2. **Week 1:** Implement 3-level hierarchical index
3. **Week 2:** Add basic knowledge graph (entities + relations)
4. **Week 3:** Query router + integration with ast-index
5. **Week 4:** Evaluation + iteration

---

## Research Documents

Full research available in:
- `/docs/knowledge-compression/README.md` — Initial problem statement
- `/knowledge-graph-research.md` — Entity-centric KG research (1000+ lines)
- Agent outputs in `/tmp/` — Raw research data

---

## Sources Summary

### Academic
- RAPTOR (Stanford): Hierarchical summarization
- GraphRAG (Microsoft): Community detection + LLM
- LLMLingua (Microsoft): Prompt compression
- RECOMP (ICLR 2024): Selective augmentation

### Tools
- LlamaIndex, LangChain: RAG frameworks
- Neo4j, SQLite: Graph storage
- Cohere Rerank: Commercial reranking
- spaCy, GliNER2: Entity extraction

### Market
- Glean ($7.2B): Enterprise knowledge
- Notion AI, Confluence AI: Workspace AI
- Cursor, GitHub Copilot: Code assistants
