pub fn build_extraction_prompt(chunk_text: &str) -> String {
    format!(
        r#"Extract entities and relationships from the following text.

INSTRUCTIONS:
1. Identify key entities (people, organizations, concepts, technologies, locations, events)
2. Extract relationships between entities
3. Output ONLY valid JSON, nothing else
4. Use the exact schema below

SCHEMA:
{{
  "entities": [
    {{"id": "E1", "name": "EntityName", "type": "PERSON|ORGANIZATION|CONCEPT|TECHNOLOGY|LOCATION|EVENT", "description": "brief description"}}
  ],
  "relations": [
    {{"source": "E1", "target": "E2", "relation": "relationship_type", "evidence": "quote from text"}}
  ]
}}

RULES:
- Use sequential IDs: E1, E2, E3, etc.
- Entity types must be one of: PERSON, ORGANIZATION, CONCEPT, TECHNOLOGY, LOCATION, EVENT
- Relation types should be verbs: "creates", "uses", "affects", "manages", "contains", etc.
- Evidence must be a direct quote from the text
- Extract 3-10 entities and 2-8 relations
- Output ONLY the JSON object, no markdown, no explanations

TEXT:
{}

JSON OUTPUT:"#,
        chunk_text
    )
}

pub fn build_retry_prompt(invalid_json: &str) -> String {
    format!(
        r#"The following JSON is invalid:

{}

Fix this JSON. Output only valid JSON with no markdown formatting, no code blocks, no explanations. Just the raw JSON object."#,
        invalid_json
    )
}