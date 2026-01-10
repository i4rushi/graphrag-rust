---

# GraphRAG in Rust: A Structured Retrieval System for LLM Reasoning

## Overview

This project implements a **GraphRAG (Graph-augmented Retrieval-Augmented Generation) system in Rust**, built to explore how **explicit structure, representation choices, and retrieval strategies** affect LLM performance, latency, and reasoning quality.

Rather than treating retrieval as a purely vector-similarity problem, this system **extracts a knowledge graph from text**, organizes entities into communities, and uses graph-aware retrieval to support:

* **Local, grounded reasoning** (entity-centric, multi-hop)
* **Global, thematic synthesis** (community-level summaries)

The project emphasizes **ML systems thinking**: pipeline design, failure modes, evaluation, and iterative improvement—rather than only maximizing benchmark scores.

---

## High-Level Architecture

The system is composed of four primary subsystems:

1. **Ingestion & Representation**
2. **Knowledge Graph Construction**
3. **Graph-Aware Retrieval**
4. **Evaluation & Benchmarking**

Each subsystem is modular, observable, and independently tunable.

---

## 1. Ingestion & Text Representation

### Document Processing

* Supports unstructured text sources (TXT / Markdown)
* Documents are chunked into semantically coherent spans using:

  * paragraph and heading boundaries
  * overlap to preserve cross-chunk context

Each chunk is assigned:

* a stable document ID
* a chunk ID
* source metadata

### Embedding Index

* Each chunk is embedded and stored in a vector database
* Payloads include references to extracted entities, enabling **vector → graph alignment**

**Why this matters (ML perspective):**
This design decouples **semantic similarity** (vector space) from **relational structure** (graph), allowing each to be optimized independently.

---

## 2. Knowledge Graph Construction

### Entity & Relation Extraction

For each chunk, an LLM is prompted to extract:

* **Entities** (name, type, description)
* **Relations** (source, target, relation type, textual evidence)

Outputs are validated against a strict JSON schema and retried on failure.

### Entity Resolution

Extracted entities are normalized and deduplicated using:

* string normalization
* similarity heuristics
* alias tracking

This prevents graph fragmentation caused by surface-form variation.

### Graph Assembly

* Entities become nodes
* Relations become typed edges
* Evidence spans are retained for traceability

**Why this matters:**
Unlike vanilla RAG, which loses relational structure at indexing time, this system **preserves explicit semantic links**, enabling multi-hop reasoning and interpretable retrieval paths.

---

## 3. Community Detection & Hierarchical Summarization

### Graph Clustering

Once the knowledge graph is constructed, entities are grouped into **communities** using graph connectivity patterns.

These communities represent:

* coherent topics
* tightly related concepts
* recurring themes across documents

### Community Summaries

For each community:

* representative entities and relations are selected
* an LLM generates a **community-level summary**
* summaries are embedded and indexed separately

Optionally, summaries can be recursively summarized to form a **hierarchical abstraction**.

**Why this matters:**
This step enables **global retrieval** that operates over *ideas* rather than raw text, addressing a known limitation of chunk-level RAG for synthesis tasks.

---

## 4. Query Engine (Three Retrieval Modes)

### A. Vanilla RAG

1. Embed query
2. Retrieve top-K chunks by vector similarity
3. Assemble context
4. Generate answer

Serves as a baseline for comparison.

---

### B. GraphRAG Local (Entity-Centric)

Designed for **fact-finding and multi-hop questions**.

1. Vector retrieval to find relevant chunks
2. Identify candidate entities referenced in those chunks
3. Expand the graph 1–2 hops from those entities
4. Gather:

   * entity descriptions
   * relational edges
   * supporting evidence
5. Generate a grounded answer with explicit traceability

**Strength:** precise, interpretable reasoning
**Cost:** higher latency due to graph traversal and context construction

---

### C. GraphRAG Global (Community-Centric)

Designed for **high-level synthesis and thematic questions**.

1. Embed query
2. Retrieve top-K community summaries
3. Optionally retrieve representative entities
4. Generate a synthesized response over abstracted knowledge

**Strength:** abstraction and theme-level reasoning
**Current limitation:** sensitive to community quality and summary faithfulness

---

## 5. Benchmarking & Evaluation

### Evaluation Dimensions

* **Latency**: Avg / P50 / P95
* **Answer Quality**: manual scoring (0–1)
* **Retrieval Mode Comparison**

### Current Benchmark Results (10 Questions)

* Vanilla RAG:

  * Avg Latency: 4.8s
  * Avg Quality: 0.65

* GraphRAG Local:

  * Avg Latency: 7.5s
  * Avg Quality: 0.65

* GraphRAG Global:

  * Avg Latency: 35.2s
  * Avg Quality: 0.50

### Interpretation

* GraphRAG Local achieves **quality parity** with Vanilla RAG while introducing structured reasoning.
* Increased latency reflects intentional design tradeoffs rather than inefficiency.
* GraphRAG Global is the most experimental component and is actively being refined.

---

## Ongoing Improvements (Active Development)

This project is iterative by design. Current focus areas include:

* Improving entity extraction precision
* Graph pruning to reduce noise
* Confidence-weighted edges
* Community detection tuning
* Smarter routing between Local and Global modes
* Latency optimizations (batching, caching)
* Expanded and category-specific evaluation sets

---

## Why This Project Is Relevant for ML Roles

This project demonstrates:

* **End-to-end ML system design**
* Thoughtful **representation learning tradeoffs**
* Practical experience with **LLM failure modes**
* Structured retrieval beyond embeddings
* Evaluation beyond “it seems better”
* Systems-level thinking in a real pipeline

It reflects how modern ML systems are built: **iteratively, with measurement, and with explicit attention to structure and assumptions**.

---

## Status

🚧 Actively under development
Benchmarks and architecture will continue to evolve as improvements are integrated.

---