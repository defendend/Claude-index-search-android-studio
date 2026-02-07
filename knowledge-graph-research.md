# Entity-Centric Knowledge Graphs for AI Context Optimization

## Executive Summary

This research explores entity-centric knowledge graphs (KGs) as an alternative to document-based retrieval for AI context optimization. The goal is to store knowledge as graphs of entities with attributes and relations rather than documents, enabling more efficient and precise context retrieval for LLMs.

**Key Finding**: GraphRAG systems achieve 80% accuracy vs 50.83% for traditional RAG, with up to 35% improvement in answer precision. Knowledge graphs can reduce context size dramatically (80 tokens vs 6000 tokens for 3 documents) when properly optimized.

---

## 1. Knowledge Graph Construction

### 1.1 Automatic KG Building from Text

Modern KG construction relies on two main approaches:

#### **LLM-Based Extraction** (State-of-the-art)
- **Entity Extraction**: Language models like GPT-4o, Claude, or specialized models extract entities directly from unstructured text without predefined labels
- **Relation Extraction**: LLMs identify meaningful relationships between entities with contextual understanding
- **Tools**:
  - **Relik**: Blazing fast framework from Sapienza University for information extraction
  - **KGGen**: Uses language models to create high-quality graphs and cluster related entities to reduce sparsity

#### **Traditional NLP Pipeline**
- **Named Entity Recognition (NER)**: Identify entities using BERT-based models or transformers
- **Relation Extraction**: Pattern matching, dependency parsing, or supervised learning
- **Entity Linking**: Connect extracted entities to canonical forms

### 1.2 Microsoft GraphRAG Construction Pipeline

Microsoft's GraphRAG represents the current state-of-the-art:

```
1. Text Chunking → Split corpus into TextUnits (analyzable chunks)
2. LLM Extraction → Extract entities, relationships, and key claims
3. Knowledge Graph → Build entity knowledge graph from extractions
4. Community Detection → Use Leiden algorithm to detect hierarchical communities
5. Summarization → Generate community summaries bottom-up across hierarchy
6. Indexing → Store graph structure + summaries for retrieval
```

**Why this works**: Creates both local (entity-level) and global (community-level) understanding of the corpus.

### 1.3 Entity Resolution & Deduplication

Critical for maintaining graph quality:

**Challenge**: Same entity, different names (e.g., "Service X", "ServiceX", "service-x")

**Solutions**:
- **Semantic Entity Resolution**: Use embeddings to find similar entities
- **Clustering Algorithms**: DBSCAN, correlation clustering for grouping duplicates
- **String Similarity**: Levenshtein distance, fuzzy matching
- **LLM-Based Resolution**: Ask LLM to determine if two entities are the same

**Implementation Pattern**:
```python
# After each extraction run
1. Generate embeddings for new entities
2. Compare with existing entity embeddings
3. If similarity > threshold, merge entities
4. Preserve temporal history (don't delete old references)
```

**Neo4j Entity Resolution**: Built-in entity resolver merges nodes with same label and name property automatically.

---

## 2. Graph Databases & Storage

### 2.1 Full-Featured Graph Databases

#### **Neo4j** (Property Graph, Most Popular)
- **Model**: Labeled Property Graph (LPG)
- **Query**: Cypher
- **Strengths**: Native graph storage, excellent tooling, strong community
- **Use Case**: Production systems, large-scale graphs (millions of nodes)

#### **Amazon Neptune** (Cloud, Multi-Model)
- **Models**: Property Graph + RDF
- **Query**: Cypher, Gremlin, SPARQL
- **Strengths**: Managed service, serverless, high availability
- **Use Case**: AWS-centric deployments

#### **FalkorDB** (Redis-Based)
- **Model**: Property Graph
- **Query**: Cypher
- **Strengths**: In-memory speed, Redis ecosystem integration
- **Use Case**: Real-time applications requiring low latency

### 2.2 Lightweight Embedded Options

Perfect for CLI tools, desktop apps, or small-to-medium datasets:

#### **LiteGraph**
- Property graph database with vectors support
- Can run in-process or as standalone server
- Designed for AI/knowledge applications
- SQLite-like simplicity

#### **FalkorDBLite**
- Embedded Python graph database
- Zero network overhead, no configuration
- Process isolation
- Ideal for prototyping and testing

#### **Cozo**
- Graph DB with Datalog queries
- Written in Rust, embedded like SQLite
- Lightweight and performant
- File-based storage

#### **SQLite with Graph Extensions**
- **simple-graph**: Pure SQLite graph implementation
- **Grafito**: Serverless SQLite-based graph database
- **Advantages**:
  - No server setup
  - Single file database
  - SQL + JSON support for complex queries
  - Perfect for 100-1000 documents
  - Easy deployment with CLI tools

**Recommendation for ast-index**: Start with SQLite-based solution (simple-graph or custom schema) for minimal dependencies and easy distribution.

### 2.3 Property Graph vs RDF Triple Store

| Aspect | Property Graph | RDF Triple Store |
|--------|---------------|------------------|
| **Model** | Nodes + Relationships with properties | Subject-Predicate-Object triples |
| **Query Language** | Cypher (GQL) | SPARQL |
| **Identifiers** | Internal IDs, string keys | URIs for everything |
| **Properties** | Rich key-value pairs on nodes/edges | Limited (need reification for edge properties) |
| **Standards** | Less standardized, more pragmatic | W3C standards, high interoperability |
| **Reasoning** | Limited, explicit traversal | Logical inference, ontology reasoning |
| **Performance** | Fast traversals, optimized for apps | Slower, designed for data exchange |
| **Learning Curve** | Easier, intuitive | Steeper, formal semantics |
| **Use Case** | Application databases, analytics | Semantic web, knowledge integration |

**For AI Context Optimization**: Property graphs are better - more expressive, faster traversals, easier to query.

---

## 3. Graph Query Languages

### 3.1 Cypher (Property Graphs)

**Pattern Matching Syntax**:
```cypher
// Find service and its dependencies
MATCH (s:Service {name: 'Service X'})
OPTIONAL MATCH (s)-[:DEPENDS_ON]->(dep)
OPTIONAL MATCH (caller)-[:CALLS]->(s)
RETURN s, collect(dep) as dependencies, collect(caller) as callers
```

**Optimization Patterns**:
- Use PROFILE to analyze query plans
- Index frequently queried properties
- Limit result sets early
- Use OPTIONAL MATCH for optional paths
- Cache execution plans (Neo4j does this automatically)

### 3.2 SPARQL (RDF Triple Stores)

**Triple Pattern Syntax**:
```sparql
SELECT ?service ?dependency WHERE {
  ?service rdf:type :Service .
  ?service :name "Service X" .
  OPTIONAL { ?service :dependsOn ?dependency }
}
```

**Optimization Patterns**:
- Reorder triple patterns by selectivity (most selective first)
- Push FILTER conditions early
- Minimize intermediate result sizes
- Use property paths for transitive queries

### 3.3 Performance Comparison

**Query Optimization Results**:
- Cypher query optimization can reduce runtime by 4.5x (from 364s to 80s)
- Triple pattern reordering is critical for SPARQL performance
- Both benefit from: selectivity estimation, early filtering, index usage

**General Principle**: Order patterns from most to least selective to minimize intermediate results.

---

## 4. Graph + LLM Integration

### 4.1 GraphRAG (Microsoft)

**Architecture**:
```
Index Time:
Text → Entities + Relations → Knowledge Graph → Community Detection → Hierarchical Summaries

Query Time:
User Query → Relevant Communities → Map-Reduce Summarization → LLM Context → Answer
```

**Key Innovations**:
1. **Hierarchical Community Detection**: Leiden algorithm recursively partitions graph into communities at multiple levels
2. **Bottom-Up Summarization**: LLM generates summaries for each community, higher levels incorporate lower summaries
3. **Map-Reduce Query**: Summaries independently answer query (map), then combine answers (reduce)

**Performance**:
- 80% accuracy vs 50.83% for traditional RAG
- 90% including acceptable answers vs 67.5% for vector RAG
- 20% better on multi-hop reasoning tasks
- Excels at "global" questions about entire datasets

**Trade-offs**:
- Higher upfront cost (graph construction)
- Slower query time (graph traversal)
- But: Better accuracy, explainability, complex reasoning

### 4.2 Graphiti (Zep AI)

**Focus**: Temporal, real-time knowledge graphs for AI agents

**Key Features**:
1. **Bi-Temporal Model**: Tracks both event occurrence time and ingestion time
2. **Real-Time Incremental Updates**: No batch recomputation, immediate entity/relation updates
3. **Temporal Queries**: Point-in-time queries, historical accuracy
4. **Non-Lossy Updates**: Preserves history when conflicts arise, doesn't discard outdated info

**Architecture**:
```
Data Stream → Entity/Relation Extraction → Entity Resolution → Graph Update → Temporal Index
                                                                         ↓
                                                            Query Time: Temporal Reasoning
```

**Use Cases**:
- AI assistants that evolve with user interactions
- Agents processing state changes from dynamic sources
- Applications requiring "memory" of past states

**Supported Databases**: Neo4j, FalkorDB, AWS Neptune

### 4.3 Context Graphs (TrustGraph)

**Optimization Focus**: AI-optimized knowledge graphs for LLMs

**Key Principles**:
- **Token Efficiency**: Maximize information density per token
- **Relevance Ranking**: Pre-rank entities/relations by importance
- **Provenance Tracking**: Maintain source references
- **Hallucination Reduction**: Ground responses in verified graph data

**Design Pattern**:
```
Entity: {
  id: "service-x"
  type: "Service"
  properties: { owner: "team-a", status: "healthy" }  // Minimal, high-value
  relations: [
    { type: "depends_on", target: "postgres", weight: 0.9 }
    { type: "depends_on", target: "redis", weight: 0.7 }
  ]
  provenance: ["doc-123:para-5", "incident-42"]
}
```

---

## 5. Incremental Updates

### 5.1 Challenge

**Problem**: Keeping KG synchronized with source data without full regeneration
- Full regeneration is time-consuming (hours for large corpora)
- Loses temporal history
- Wastes computing resources

### 5.2 Solutions

#### **IncRML (Incremental RML)**
- Detects changes in source data using hash-based strategies
- Only processes changed portions
- **Performance**: 315x less storage, 4.6x less CPU, 4.4x faster construction

**Algorithm**:
```
1. Hash each source record
2. Store hashes alongside graph data
3. On sync:
   - Compute new hashes
   - Compare with stored hashes
   - Only process changed records
4. Update affected graph portions
```

#### **Change Data Capture (CDC)**
- Stream changes from source systems (e.g., Kafka, database CDC)
- Process changes incrementally
- Neo4j native CDC: FULL mode (before/after state) or DIFF mode (only changes)

#### **Hash-Based Change Detection**
```
Hierarchical hashing:
- Document hash (top level)
- Section hashes (mid level)
- Paragraph hashes (leaf level)

If document hash unchanged → skip
If section hash changed → only reprocess that section
```

#### **Event-Driven Architecture**
- Source systems emit events on change
- Graph update service consumes events
- Real-time or near-real-time updates

### 5.3 Best Practices

1. **Versioning**: Tag graph elements with version/timestamp
2. **Soft Deletes**: Mark entities as inactive rather than deleting
3. **Temporal Tracking**: Preserve history (when entity/relation was valid)
4. **Batch Micro-Updates**: Group small changes, process periodically
5. **Conflict Resolution**: Use timestamps to determine which update wins

**Example Pattern (Graphiti-style)**:
```python
class Entity:
    valid_from: datetime
    valid_until: datetime | None  # None = still valid

def update_entity(entity_id, new_data, event_time):
    existing = get_entity(entity_id)
    if existing:
        existing.valid_until = event_time
    create_entity(new_data, valid_from=event_time)
```

---

## 6. Schema Design for Universal Knowledge Graph

### 6.1 Principles

**Universal vs Domain-Specific**: Balance between general applicability and domain precision

**Core Pattern**: Entity-Attribute-Relation (EAR)
```
Entity: Thing with identity (node)
Attribute: Property of entity (key-value on node)
Relation: Connection between entities (edge)
```

### 6.2 Universal Schema Examples

#### **Biolink Model** (Biomedical)
- Universal data model for translational science
- Defines entities (genes, diseases, drugs) and relationships
- Open-source, supports data interoperability

#### **KBpedia** (General Knowledge)
- Integrates 7 public knowledge bases: Wikipedia, Wikidata, schema.org, DBpedia, GeoNames, OpenCyc, UNSPSC
- 50,000+ concepts, 2 million+ entities
- Designed for AI interoperability

#### **Schema.org** (Web Semantics)
- Widely adopted for structured web data
- Hierarchical types (Thing → CreativeWork, Organization, Person, etc.)
- Used by Google, Microsoft, Yandex for search

### 6.3 Proposed Universal Schema for Code/Systems

**Entity Types**:
```
CodeEntity
  ├── Module (file, package)
  ├── Class
  ├── Function/Method
  ├── Variable
  └── Type

SystemEntity
  ├── Service
  ├── Database
  ├── API
  └── Configuration

DocumentEntity
  ├── Specification
  ├── Design Doc
  └── Runbook

OperationalEntity
  ├── Incident
  ├── Deployment
  └── Metric
```

**Relation Types**:
```
Code Relations:
  - calls, implements, extends, uses, imports
  - defines, references, modifies

System Relations:
  - depends_on, communicates_with, stores_in, deployed_to
  - monitored_by, owns, maintains

Cross-Domain Relations:
  - documents, related_to, caused_by, fixed_in
```

**Attributes (Examples)**:
```
Common: name, description, created_at, updated_at, source_file
Code: line_number, signature, visibility, language
System: status, owner_team, criticality, version
Operational: severity, start_time, end_time, affected_users
```

### 6.4 Ontology Design Patterns (ODPs)

**Reusable patterns** for common modeling scenarios:

1. **Part-Whole**: Component relationships (Service has Database)
2. **Agent-Role**: Who does what (Team owns Service)
3. **Event Provenance**: What happened when (Deployment caused Incident)
4. **Measurement**: Metrics and values (Service has Latency)
5. **Classification**: Taxonomies (Service is-a SystemEntity)

**Modular Ontology Modeling (MOMo)**: Build ontology from composable patterns, easier to extend and maintain.

---

## 7. Efficient Graph Querying

### 7.1 Query Patterns for Context Retrieval

#### **Pattern 1: Entity-Centric Neighborhood**
```cypher
// Get entity + immediate context
MATCH (e:Entity {id: $entity_id})
OPTIONAL MATCH (e)-[r]->(related)
RETURN e, collect({relation: type(r), target: related}) as context
LIMIT 50
```

**Token Efficiency**: ~80 tokens vs 2000 tokens for full document

#### **Pattern 2: Multi-Hop Traversal**
```cypher
// Find path between entities
MATCH path = (start:Entity {id: $start_id})-[*1..3]-(end:Entity {id: $end_id})
RETURN path
ORDER BY length(path)
LIMIT 10
```

**Use Case**: "How does Service A interact with Database B?"

#### **Pattern 3: Subgraph Extraction**
```cypher
// Extract subgraph around entity
MATCH (center:Service {name: $service_name})
CALL apoc.path.subgraphAll(center, {
    relationshipFilter: "DEPENDS_ON|CALLS|STORES_IN",
    maxLevel: 2
})
YIELD nodes, relationships
RETURN nodes, relationships
```

**Token Efficiency**: Only relevant subgraph, not entire corpus

#### **Pattern 4: Community-Based Retrieval (GraphRAG Style)**
```cypher
// Find relevant community summaries
MATCH (c:Community)
WHERE c.summary_embedding <similar_to> $query_embedding
RETURN c.summary, c.member_count
ORDER BY similarity DESC
LIMIT 5
```

**Use Case**: Global questions about system architecture

### 7.2 Optimization Techniques

#### **Indexing Strategy**
```cypher
// Neo4j indexes
CREATE INDEX entity_name FOR (e:Entity) ON (e.name)
CREATE INDEX entity_type FOR (e:Entity) ON (e.type)
CREATE FULLTEXT INDEX entity_search FOR (e:Entity) ON EACH [e.name, e.description]
```

#### **Query Rewriting Rules**
1. **Push predicates down**: Filter early in query
2. **Limit early**: Don't retrieve more than needed
3. **Use OPTIONAL MATCH**: For optional paths (avoids Cartesian products)
4. **Batch queries**: Multiple simple queries > one complex query with Cartesian product

#### **Caching**
- **Query Plan Cache**: Neo4j automatically caches execution plans
- **Result Cache**: Cache frequent queries (e.g., service metadata)
- **Embedding Cache**: Pre-compute and store embeddings for entities

### 7.3 Token Efficiency Analysis

**Scenario**: "What do we know about Service X?"

**Document-Based RAG**:
```
Retrieve: 3 relevant documents × 2000 tokens = 6000 tokens
Context window used: 6000 tokens
Relevant info: ~200 tokens (scattered across docs)
Efficiency: 3.3%
```

**Graph-Based RAG**:
```
Query: Get Service X entity + relations
Return:
  - Service X attributes: 20 tokens
  - Dependencies (3): 30 tokens
  - Callers (2): 20 tokens
  - Recent incident: 30 tokens
  - Total: 100 tokens
Context window used: 100 tokens
Relevant info: 100 tokens
Efficiency: 100%
```

**Improvement**: 60x reduction in tokens, 30x increase in relevance density

---

## 8. When to Use Graphs vs Alternatives

### 8.1 Graph Excels

**Use Cases**:
1. **Relationship-Centric Queries**: "How does X relate to Y?"
2. **Multi-Hop Reasoning**: "What's the path from A to D?"
3. **Entity-Focused Retrieval**: "Tell me about Service X"
4. **Temporal Reasoning**: "What changed between version 1 and 2?"
5. **Deduplication/Entity Resolution**: Canonical entity representation
6. **Global Questions**: "Summarize all services owned by Team A"

**Advantages**:
- Explicit relationship modeling
- Efficient traversals (O(1) for graph databases)
- Token efficiency for structured knowledge
- Easier to update incrementally
- Better explainability (follow path in graph)

### 8.2 Document/Vector RAG Excels

**Use Cases**:
1. **Keyword/Semantic Search**: "Find documents about authentication"
2. **Long-Form Content**: Full text of articles, docs
3. **Fuzzy Matching**: Similar documents, conceptual overlap
4. **No Clear Entities**: Narrative text, opinions, discussions

**Advantages**:
- Simpler to implement (no entity extraction)
- Better for unstructured narrative text
- Captures nuance and context better
- Faster to build (no graph construction)

### 8.3 Hierarchical Index Excels

**Use Cases**:
1. **Code Navigation**: File → Class → Method hierarchy
2. **Taxonomy-Based Retrieval**: Category browsing
3. **Exact Lookups**: "Jump to definition"

**Advantages**:
- Natural fit for tree-structured data
- Fast exact lookups (O(log n))
- Simple to implement and understand
- Low overhead

### 8.4 Hybrid Approach (Recommended)

**Combine strengths of each**:

```
Layer 1: Hierarchical Index
  - Fast exact lookups (file, function, class)
  - Navigate tree structure

Layer 2: Knowledge Graph
  - Relationships (calls, imports, depends_on)
  - Entity attributes and metadata
  - Temporal information

Layer 3: Vector Embeddings
  - Semantic search over descriptions
  - Find similar entities
  - Fuzzy matching

Layer 4: Document Store
  - Full text of documentation
  - Comments and descriptions
  - Historical context
```

**Query Strategy**:
```
1. Exact match? → Hierarchical index
2. Entity + relationships? → Knowledge graph
3. Semantic search? → Vector embeddings
4. Need full context? → Document store
```

**Benefits**: Best of all worlds, optimized for different query types

---

## 9. Implementation Proposal for ast-index

### 9.1 Recommended Architecture

**Storage**: SQLite with graph schema (lightweight, embeddable)

**Schema**:
```sql
-- Entities
CREATE TABLE entities (
    id INTEGER PRIMARY KEY,
    type TEXT NOT NULL,  -- Function, Class, Module, etc.
    name TEXT NOT NULL,
    file_path TEXT NOT NULL,
    line_number INTEGER,
    signature TEXT,
    description TEXT,
    embedding BLOB,  -- Optional: for semantic search
    metadata JSON,   -- Flexible key-value storage
    created_at INTEGER,
    updated_at INTEGER
);

-- Relations
CREATE TABLE relations (
    id INTEGER PRIMARY KEY,
    source_id INTEGER NOT NULL,
    target_id INTEGER NOT NULL,
    relation_type TEXT NOT NULL,  -- calls, imports, extends, etc.
    metadata JSON,
    created_at INTEGER,
    FOREIGN KEY (source_id) REFERENCES entities(id),
    FOREIGN KEY (target_id) REFERENCES entities(id)
);

-- Indexes
CREATE INDEX idx_entity_type ON entities(type);
CREATE INDEX idx_entity_name ON entities(name);
CREATE INDEX idx_entity_file ON entities(file_path);
CREATE INDEX idx_relation_type ON relations(relation_type);
CREATE INDEX idx_relation_source ON relations(source_id);
CREATE INDEX idx_relation_target ON relations(target_id);
CREATE UNIQUE INDEX idx_entity_identity ON entities(type, name, file_path);
```

### 9.2 Construction Pipeline

```rust
// High-level pipeline
fn build_knowledge_graph(repo_path: &Path) -> Result<KnowledgeGraph> {
    let mut graph = KnowledgeGraph::new();

    // 1. Extract entities (existing indexing)
    for file in discover_files(repo_path) {
        let ast = parse_file(&file)?;
        let entities = extract_entities(&ast, &file);
        graph.add_entities(entities);
    }

    // 2. Extract relations
    for file in discover_files(repo_path) {
        let ast = parse_file(&file)?;
        let relations = extract_relations(&ast, &file, &graph);
        graph.add_relations(relations);
    }

    // 3. Entity resolution
    graph.resolve_duplicates()?;

    // 4. Compute embeddings (optional)
    if config.enable_semantic_search {
        graph.compute_embeddings()?;
    }

    Ok(graph)
}
```

### 9.3 Query Patterns

**Pattern 1: Entity Lookup**
```rust
fn get_entity_context(graph: &KG, entity_id: i64, max_depth: usize) -> String {
    let entity = graph.get_entity(entity_id)?;
    let outgoing = graph.get_relations(entity_id, Direction::Outgoing, max_depth);
    let incoming = graph.get_relations(entity_id, Direction::Incoming, max_depth);

    format_compact_context(entity, outgoing, incoming)
}
```

**Pattern 2: Path Finding**
```rust
fn find_path(graph: &KG, start: &str, end: &str) -> Vec<Path> {
    // Dijkstra or BFS to find shortest paths
    let start_entity = graph.find_entity_by_name(start)?;
    let end_entity = graph.find_entity_by_name(end)?;

    graph.shortest_paths(start_entity.id, end_entity.id, max_length: 5)
}
```

**Pattern 3: Subgraph Extraction**
```rust
fn extract_subgraph(graph: &KG, center: &str, radius: usize) -> Subgraph {
    let entity = graph.find_entity_by_name(center)?;
    let mut subgraph = Subgraph::new();

    // BFS from center up to radius hops
    let mut queue = VecDeque::from([(entity.id, 0)]);
    let mut visited = HashSet::new();

    while let Some((node_id, depth)) = queue.pop_front() {
        if depth > radius || visited.contains(&node_id) {
            continue;
        }
        visited.insert(node_id);

        let node = graph.get_entity(node_id)?;
        subgraph.add_entity(node);

        for rel in graph.get_relations(node_id, Direction::Both, 1) {
            subgraph.add_relation(rel);
            queue.push_back((rel.target_id, depth + 1));
        }
    }

    subgraph
}
```

### 9.4 Incremental Updates

**File-Level Change Detection**:
```rust
struct FileHash {
    file_path: String,
    content_hash: u64,
    last_modified: SystemTime,
}

fn incremental_update(graph: &mut KG, changed_files: Vec<PathBuf>) -> Result<()> {
    for file in changed_files {
        // 1. Delete old entities/relations from this file
        graph.delete_entities_from_file(&file);

        // 2. Re-extract from file
        let ast = parse_file(&file)?;
        let entities = extract_entities(&ast, &file);
        let relations = extract_relations(&ast, &file, &graph);

        // 3. Add back to graph
        graph.add_entities(entities);
        graph.add_relations(relations);

        // 4. Entity resolution (for cross-file references)
        graph.resolve_entities_in_file(&file)?;
    }

    Ok(())
}
```

**Watch Mode**:
```rust
fn watch_and_update(graph: Arc<Mutex<KG>>, repo_path: PathBuf) {
    let (tx, rx) = channel();
    let mut watcher = notify::watcher(tx, Duration::from_secs(1))?;
    watcher.watch(repo_path, RecursiveMode::Recursive)?;

    loop {
        match rx.recv() {
            Ok(DebouncedEvent::Write(path)) => {
                let mut graph = graph.lock().unwrap();
                incremental_update(&mut graph, vec![path])?;
            }
            _ => {}
        }
    }
}
```

### 9.5 Query Interface

**CLI Commands**:
```bash
# Entity lookup
ast-index graph get "function_name"

# Show relationships
ast-index graph relations "MyClass"

# Find path
ast-index graph path "function_a" "function_b"

# Extract subgraph
ast-index graph subgraph "MyService" --depth 2

# Export for LLM
ast-index graph export "MyService" --format compact --max-tokens 500
```

**Compact Format for LLM Context**:
```
Entity: MyService (Class)
  File: src/services/my_service.rs:45
  Methods: [start, stop, handle_request]

Dependencies:
  → Database (stores_in)
  → Logger (uses)

Called by:
  ← ApiHandler (calls)
  ← TestSuite (tests)

Recent changes:
  - Added handle_request method (2025-02-01)
  - Fixed connection leak (2025-01-28)

Total: 95 tokens
```

---

## 10. Comparison with Alternatives

### 10.1 Graph vs Hierarchical Index

| Aspect | Knowledge Graph | Hierarchical Index |
|--------|----------------|-------------------|
| **Structure** | Network (arbitrary connections) | Tree (parent-child) |
| **Relationships** | Many-to-many, typed | One-to-many (parent) |
| **Queries** | Traversals, path finding | Lookups, tree navigation |
| **Flexibility** | High (add any relation) | Low (fixed hierarchy) |
| **Complexity** | Higher (graph algorithms) | Lower (simple traversal) |
| **Token Efficiency** | Very high (only relevant subgraph) | High (only relevant branch) |
| **Update Cost** | Medium (relation updates) | Low (local updates) |
| **Best For** | Relationships, reasoning | Exact lookups, navigation |

**Verdict**: Graphs are more powerful but more complex. Use hierarchical for exact lookups, graph for relationships.

### 10.2 Graph vs Document Chunks

| Aspect | Knowledge Graph | Document Chunks |
|--------|----------------|-----------------|
| **Granularity** | Entity-level | Paragraph/section level |
| **Structure** | Explicit (nodes/edges) | Implicit (text proximity) |
| **Retrieval** | Precise (exact entity) | Approximate (semantic search) |
| **Token Efficiency** | Very high (60x better) | Low (lots of irrelevant text) |
| **Context Loss** | Low (preserves relations) | High (chunk boundaries) |
| **Implementation** | Complex (entity extraction) | Simple (chunking + embeddings) |
| **Multi-hop** | Native support | Difficult (multiple retrievals) |
| **Best For** | Structured knowledge | Narrative content |

**Verdict**: Graphs dramatically more efficient for structured knowledge, but require upfront investment in entity extraction.

### 10.3 Graph vs Vector Embeddings

| Aspect | Knowledge Graph | Vector Embeddings |
|--------|----------------|------------------|
| **Search Type** | Exact + traversal | Semantic similarity |
| **Explainability** | High (follow path) | Low (black box similarity) |
| **Precision** | High (exact matches) | Medium (fuzzy matches) |
| **Recall** | Medium (must model relation) | High (finds similar concepts) |
| **Token Efficiency** | Very high | Medium |
| **Update Cost** | Medium | Low (re-embed changed docs) |
| **Best For** | Known entities, relationships | Exploratory search, similar concepts |

**Verdict**: Complementary. Use graphs for structured queries, embeddings for semantic discovery.

### 10.4 Hybrid Recommendation

**For ast-index**: Implement hybrid approach

```
1. Hierarchical Index (existing)
   → Fast file/function/class lookups

2. Knowledge Graph (new)
   → Relationships (calls, imports, etc.)
   → Entity metadata and attributes

3. Vector Embeddings (future)
   → Semantic search over descriptions
   → Find similar code patterns
```

**Query Routing Logic**:
```rust
fn route_query(query: &Query) -> QueryStrategy {
    match query.intent {
        QueryIntent::ExactLookup => QueryStrategy::HierarchicalIndex,
        QueryIntent::Relationships => QueryStrategy::KnowledgeGraph,
        QueryIntent::Semantic => QueryStrategy::VectorEmbeddings,
        QueryIntent::Complex => QueryStrategy::Hybrid,
    }
}
```

---

## 11. Key Insights & Recommendations

### 11.1 Graph Construction

1. **Use LLMs for extraction**: Modern LLM-based extraction (GPT-4o, Claude) outperforms traditional NER
2. **Entity resolution is critical**: Invest in deduplication to maintain graph quality
3. **Hierarchical communities**: GraphRAG's community detection enables both local and global reasoning
4. **Incremental updates**: Hash-based change detection is 4x faster than full regeneration

### 11.2 Storage

1. **Start lightweight**: SQLite with graph schema is sufficient for small-to-medium datasets (< 1M entities)
2. **Property graphs > RDF**: For AI/LLM use cases, property graphs are more practical
3. **Embedded > Client-Server**: For CLI tools, embedded databases (LiteGraph, Cozo) avoid deployment complexity

### 11.3 Querying

1. **Token efficiency is key**: Graphs can reduce context size by 60x vs document chunks
2. **Optimize for common patterns**: Entity lookup, relationship traversal, subgraph extraction
3. **Cache aggressively**: Query plans, frequent results, embeddings
4. **Index strategically**: Type, name, file path for entities; type, source, target for relations

### 11.4 When to Use Graphs

**Use graphs when**:
- Entities and relationships are well-defined
- Queries involve traversals or multi-hop reasoning
- Token efficiency is critical (LLM context limits)
- Explainability matters (show path in graph)

**Don't use graphs when**:
- Data is purely narrative (no clear entities)
- Simple keyword search is sufficient
- Implementation complexity outweighs benefits

### 11.5 Hybrid is Best

**Don't choose graph OR documents OR hierarchy**. Use all three:
- Hierarchy for exact lookups
- Graph for relationships
- Embeddings for semantic search
- Documents for full context

**Route queries to optimal layer based on intent.**

---

## Sources

### GraphRAG & Microsoft Research
- [GraphRAG: Unlocking LLM discovery on narrative private data](https://www.microsoft.com/en-us/research/blog/graphrag-unlocking-llm-discovery-on-narrative-private-data/)
- [GraphRAG Documentation](https://microsoft.github.io/graphrag/)
- [GraphRAG GitHub](https://github.com/microsoft/graphrag)
- [From Local to Global: A Graph RAG Approach to Query-Focused Summarization](https://arxiv.org/abs/2404.16130)
- [GraphRAG improving global search via dynamic community selection](https://www.microsoft.com/en-us/research/blog/graphrag-improving-global-search-via-dynamic-community-selection/)

### Knowledge Graph Construction
- [How to Create a Knowledge Graph from Text - Stanford](https://web.stanford.edu/class/cs520/2020/notes/How_To_Create_A_Knowledge_Graph_From_Text.html)
- [Entity Linking and Relationship Extraction With Relik in LlamaIndex](https://neo4j.com/blog/developer/entity-linking-relationship-extraction-relik-llamaindex/)
- [Knowledge Graph Construction: Extraction, Learning, and Evaluation](https://www.mdpi.com/2076-3417/15/7/3727)
- [KGGen: Extracting Knowledge Graphs from Plain Text with Language Models](https://arxiv.org/html/2502.09956v1)
- [Automated Knowledge Graph Construction using Large Language Models](https://arxiv.org/html/2509.17289v1)

### Neo4j & Entity Resolution
- [Entity Resolved Knowledge Graphs: A Tutorial](https://neo4j.com/blog/developer/entity-resolved-knowledge-graphs/)
- [Neo4j Entity Resolution GitHub Example](https://github.com/neo4j-graph-examples/entity-resolution)
- [Neo4j Data Modeling](https://hackolade.com/nosqldb/neo4j-data-modeling.html)
- [What is graph data modeling?](https://neo4j.com/docs/getting-started/data-modeling/)
- [LLM Knowledge Graph Builder — First Release of 2025](https://neo4j.com/blog/developer/llm-knowledge-graph-builder-release/)

### Incremental Updates
- [Building a Knowledge Graph from Meeting Notes that automatically updates](https://cocoindex.io/blogs/meeting-notes-graph)
- [How do you keep a knowledge graph updated?](https://milvus.io/ai-quick-reference/how-do-you-keep-a-knowledge-graph-updated)
- [IncRML: Incremental Knowledge Graph Construction](https://www.semantic-web-journal.net/content/incrml-incremental-knowledge-graph-construction-heterogeneous-data-sources)
- [Graphiti: Build Real-Time Knowledge Graphs](https://github.com/getzep/graphiti)
- [How to Handle Incremental Updates in GraphRAG](https://github.com/microsoft/graphrag/discussions/511)
- [Graph Maintenance at Scale: The Data Tax Nobody Warns You About](https://medium.com/@shereshevsky/graph-maintenance-at-scale-the-data-tax-nobody-warns-you-about-740ad51a3249)

### Query Optimization
- [Query Processing and Optimization in Graph Databases](https://mediatum.ub.tum.de/doc/1238730/1238730.pdf)
- [The SPARQL Query Graph Model for Query Optimization](https://link.springer.com/chapter/10.1007/978-3-540-72667-8_40)
- [Optimisation Techniques for Flexible SPARQL Queries](https://dl.acm.org/doi/10.1145/3532855)
- [Query tuning - Cypher Manual](https://neo4j.com/docs/cypher-manual/current/planning-and-tuning/query-tuning/)
- [Performance of graph query languages: Comparison of cypher, gremlin and native access](https://www.researchgate.net/publication/258317367_Performance_of_graph_query_languages_Comparison_of_cypher_gremlin_and_native_access_in_Neo4j)

### Lightweight Databases
- [LiteGraph GitHub](https://github.com/jchristn/LiteGraph)
- [How to Build Lightweight GraphRAG with SQLite](https://dev.to/stephenc222/how-to-build-lightweight-graphrag-with-sqlite-53le)
- [FalkorDBLite: Embedded Python Graph Database](https://www.falkordb.com/blog/falkordblite-embedded-python-graph-database/)
- [SQLite for GraphRAG](https://stephencollins.tech/newsletters/sqlite-for-graphrag-lightweight-graph-database)
- [simple-graph: Graph database in SQLite](https://github.com/dpapathanasiou/simple-graph)
- [Cozo – Graph DB with Datalog](https://news.ycombinator.com/item?id=33518320)

### Schema Design & Ontologies
- [Ontologies: Blueprints for Knowledge Graph Structures](https://www.falkordb.com/blog/understanding-ontologies-knowledge-graph-schemas/)
- [Biolink Model: Universal schema for knowledge graphs](https://pmc.ncbi.nlm.nih.gov/articles/PMC9372416/)
- [KBpedia - Open-source Integrated Knowledge Structure](https://kbpedia.org/)
- [Ontology in Graph Models and Knowledge Graphs](https://graph.build/resources/ontology)
- [Empirical ontology design patterns and shapes from Wikidata](https://journals.sagepub.com/doi/10.3233/SW-243613)

### RAG Performance Comparison
- [RAG vs. GraphRAG: A Systematic Evaluation and Key Insights](https://arxiv.org/abs/2502.11371)
- [Graph RAG vs RAG: Which One Is Truly Smarter?](https://datasciencedojo.com/blog/graph-rag-vs-rag/)
- [GraphRAG vs. Vector RAG: Side-by-side comparison](https://www.meilisearch.com/blog/graph-rag-vs-vector-rag)
- [Improving RAG accuracy with GraphRAG - AWS](https://aws.amazon.com/blogs/machine-learning/improving-retrieval-augmented-generation-accuracy-with-graphrag/)
- [RAG vs GraphRAG: Shared Goal & Key Differences](https://memgraph.com/blog/rag-vs-graphrag)
- [Knowledge graph vs. vector database for RAG](https://www.meilisearch.com/blog/knowledge-graph-vs-vector-database-for-rag)

### Entity Resolution & Deduplication
- [Entity-Resolved Knowledge Graphs](https://towardsdatascience.com/entity-resolved-knowledge-graphs-6b22c09a1442/)
- [What Is Entity Resolution? - Neo4j](https://neo4j.com/blog/graph-database/what-is-entity-resolution/)
- [Combining entity resolution and knowledge graphs](https://linkurious.com/blog/entity-resolution-knowledge-graph/)
- [What Are Entity Resolved Knowledge Graphs? - Senzing](https://senzing.com/entity-resolved-knowledge-graphs/)
- [The Rise of Semantic Entity Resolution](https://blog.graphlet.ai/the-rise-of-semantic-entity-resolution-45c48d5eb00a)
- [Data Deduplication in Scraped Knowledge Graphs](https://scrapingant.com/blog/data-deduplication-and-canonicalization-in-scraped)

### Graphiti & Temporal Graphs
- [Graphiti: Build Real-Time Knowledge Graphs for AI Agents](https://github.com/getzep/graphiti)
- [Graphiti: Knowledge Graph Memory](https://neo4j.com/blog/developer/graphiti-knowledge-graph-memory/)
- [Building Temporal Knowledge Graphs with Graphiti](https://www.falkordb.com/blog/building-temporal-knowledge-graphs-graphiti/)
- [Graphiti: Giving AI a Real Memory](https://www.presidio.com/technical-blog/graphiti-giving-ai-a-real-memory-a-story-of-temporal-knowledge-graphs/)
- [Zep: A Temporal Knowledge Graph Architecture](https://arxiv.org/html/2501.13956v1)

### Property Graph vs RDF
- [RDF Triple Stores vs. Property Graphs](https://neo4j.com/blog/knowledge-graph/rdf-vs-property-graphs-knowledge-graphs/)
- [Knowledge Graphs: RDF or Property Graphs, Which One?](https://www.wisecube.ai/blog/knowledge-graphs-rdf-or-property-graphs-which-one-should-you-pick/)
- [Property Graph vs RDF Triple Store: A Comparison](https://journals.plos.org/plosone/article?id=10.1371/journal.pone.0144578)
- [What Is a Knowledge Graph? RDF vs Property Graphs](https://taewoon.kim/2025-10-06-knowledge-graph/)
- [Property graph vs RDF – Key Differences](https://www.puppygraph.com/blog/property-graph-vs-rdf)
- [RDF vs Property Graphs - Ontotext](https://www.ontotext.com/knowledgehub/fundamentals/rdf-vs-property-graphs/)

### Token Efficiency
- [REMINDRAG: Low-Cost LLM-Guided Knowledge Graph Traversal](https://openreview.net/pdf?id=JnKfAqLJb4)
- [Beyond Chunks and Graphs: RAG](https://arxiv.org/pdf/2508.02435)
- [Chunking Strategies for LLM Applications - Pinecone](https://www.pinecone.io/learn/chunking-strategies/)
- [Context Graphs: AI-Optimized Knowledge Graphs](https://trustgraph.ai/guides/key-concepts/context-graphs/)
- [Evaluating Chunking Strategies for Retrieval](https://research.trychroma.com/evaluating-chunking)
- [LLM Chunking: Improve Retrieval & Accuracy - Redis](https://redis.io/blog/llm-chunking/)

---

## Conclusion

Entity-centric knowledge graphs offer dramatic improvements in token efficiency (60x) and accuracy (80% vs 50.83%) for AI context optimization compared to traditional document-based retrieval. The key innovations from Microsoft GraphRAG (hierarchical community detection + summarization) and Graphiti (temporal, real-time updates) provide robust patterns for building production systems.

**Recommended approach for ast-index**:
1. Start with SQLite-based graph storage (lightweight, embeddable)
2. Extract entities (functions, classes, modules) and relations (calls, imports, depends_on)
3. Implement basic graph queries (entity lookup, relationship traversal, subgraph extraction)
4. Add incremental updates using file-level hashing
5. Future: Add embeddings for semantic search (hybrid approach)

**Key insight**: Don't choose graph OR documents OR hierarchy - use all three, routing queries to the optimal layer based on intent. This hybrid approach combines the strengths of each while mitigating their weaknesses.
