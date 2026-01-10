# GraphRAG: Advanced Retrieval System

GraphRAG is a retrieval-augmented generation system developed by Microsoft Research. It combines vector embeddings with knowledge graphs to improve answer quality.

## Key Components

The system uses Qdrant as a vector database for semantic search. Qdrant stores document embeddings and enables fast similarity search across millions of vectors.

Neo4j serves as the graph database, storing entities and their relationships. The graph structure captures how concepts relate to each other, enabling both local and global queries.

## How It Works

First, documents are chunked into smaller pieces. Each chunk is processed by a large language model like GPT-4 or Claude to extract entities and relationships.

These entities are then clustered into communities using the Louvain algorithm. Community detection helps identify major themes and topics across the entire corpus.

Finally, the query engine can perform local search by expanding from relevant entities, or global search by synthesizing community summaries.

## Benefits

GraphRAG outperforms traditional RAG systems on multi-hop questions and summarization tasks. The graph structure enables better context gathering and reduces hallucinations.
EOF