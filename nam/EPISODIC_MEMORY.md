# Episodic Memory Layer (NAM-06)

**Version:** 1.0.0
**Status:** Specification Complete
**NAM Compliance:** NAM-06 (Context Recall)
**Impact:** Enable "This situation is like when..." reasoning

---

## 1. Overview

The system has semantic memory (what things are) but no episodic memory (what happened when). Episodic memory enables contextual recall and pattern matching to past experiences.

### 1.1 Philosophy

> "Those who cannot remember the past are condemned to repeat it." - George Santayana

Episodic memory allows the maintenance engine to:
- Recall similar past situations
- Learn from both successes and failures
- Build causal chains between events
- Avoid repeating past mistakes

### 1.2 Key Concepts

| Concept | Definition |
|---------|------------|
| **Episode** | A bounded sequence of events with causal structure |
| **Trigger Event** | The event that started the episode |
| **Resolution Event** | The event that ended the episode |
| **Tensor Signature** | 12D encoding of system state during episode |
| **Episode Link** | Causal or similarity relationship between episodes |

---

## 2. Database Schema (episodic_memory.db)

### 2.1 Episodes Table

```sql
-- Episodes: Bounded sequences of events with causal structure
CREATE TABLE episodes (
    episode_id TEXT PRIMARY KEY,

    -- Temporal bounds
    start_timestamp DATETIME NOT NULL,
    end_timestamp DATETIME,

    -- Event markers
    trigger_event TEXT NOT NULL,         -- Event type that triggered episode
    trigger_event_id TEXT,               -- Reference to specific event
    resolution_event TEXT,               -- Event type that resolved episode
    resolution_event_id TEXT,            -- Reference to specific event

    -- Outcome classification
    outcome TEXT CHECK(outcome IN (
        'success',    -- Episode resolved positively
        'failure',    -- Episode resolved negatively
        'partial',    -- Mixed outcome
        'ongoing',    -- Episode still in progress
        'timeout',    -- Episode timed out without resolution
        'escalated'   -- Episode escalated to higher tier
    )),

    -- Scope and context
    services_involved TEXT NOT NULL,     -- JSON array of service IDs
    pathways_activated TEXT NOT NULL,    -- JSON array of Hebbian pathway IDs
    agents_participated TEXT,            -- JSON array of agent IDs

    -- State encoding
    tensor_signature BLOB NOT NULL,      -- 12D tensor encoding at episode start
    tensor_signature_end BLOB,           -- 12D tensor encoding at episode end

    -- Narrative and annotation
    narrative TEXT,                       -- Machine-generated summary
    human_annotation TEXT,                -- Optional human notes
    emotional_valence REAL DEFAULT 0.5,  -- 0=negative, 0.5=neutral, 1=positive

    -- Learning outcomes
    lessons_learned TEXT,                 -- JSON array of insights
    pathway_deltas TEXT,                  -- JSON object of pathway changes

    -- Metadata
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Indexes for episode queries
CREATE INDEX idx_episodes_trigger ON episodes(trigger_event);
CREATE INDEX idx_episodes_outcome ON episodes(outcome);
CREATE INDEX idx_episodes_valence ON episodes(emotional_valence);
CREATE INDEX idx_episodes_start ON episodes(start_timestamp);
CREATE INDEX idx_episodes_ongoing ON episodes(outcome) WHERE outcome = 'ongoing';
```

### 2.2 Episode Links Table

```sql
-- Episode relationships (causal chains)
CREATE TABLE episode_links (
    id TEXT PRIMARY KEY,
    source_episode_id TEXT NOT NULL,
    target_episode_id TEXT NOT NULL,
    link_type TEXT NOT NULL CHECK(link_type IN (
        'caused',       -- Source episode caused target episode
        'prevented',    -- Source episode prevented target episode
        'similar_to',   -- Episodes are similar (not causal)
        'opposite_of',  -- Episodes had opposite outcomes
        'preceded',     -- Source preceded target (temporal)
        'escalated_to'  -- Source escalated to target
    )),
    strength REAL NOT NULL DEFAULT 0.5,  -- Link strength [0.0, 1.0]
    evidence TEXT,                        -- JSON describing why link exists
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,

    PRIMARY KEY (source_episode_id, target_episode_id),
    FOREIGN KEY (source_episode_id) REFERENCES episodes(episode_id),
    FOREIGN KEY (target_episode_id) REFERENCES episodes(episode_id)
);

CREATE INDEX idx_links_source ON episode_links(source_episode_id);
CREATE INDEX idx_links_target ON episode_links(target_episode_id);
CREATE INDEX idx_links_type ON episode_links(link_type);
```

### 2.3 Episode Events Table

```sql
-- Events within an episode
CREATE TABLE episode_events (
    id TEXT PRIMARY KEY,
    episode_id TEXT NOT NULL,
    event_type TEXT NOT NULL,
    event_data TEXT NOT NULL,            -- JSON
    tensor_snapshot BLOB,                -- 12D tensor at event time
    sequence_number INTEGER NOT NULL,    -- Order within episode
    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,

    FOREIGN KEY (episode_id) REFERENCES episodes(episode_id)
);

CREATE INDEX idx_episode_events_episode ON episode_events(episode_id);
CREATE INDEX idx_episode_events_type ON episode_events(event_type);
```

---

## 3. Episode Recording

### 3.1 Episode Recording Implementation

```rust
/// Episodic memory system for maintaining event history
pub struct EpisodicMemory {
    db: Pool<Sqlite>,
    active_episodes: DashMap<String, ActiveEpisode>,
}

#[derive(Debug, Clone)]
pub struct ActiveEpisode {
    pub episode_id: String,
    pub trigger_event: String,
    pub start_timestamp: DateTime<Utc>,
    pub services_involved: Vec<String>,
    pub pathways_activated: Vec<String>,
    pub events: Vec<EpisodeEvent>,
    pub tensor_signature: MaintenanceTensor,
}

impl EpisodicMemory {
    /// Start recording a new episode
    pub async fn start_episode(&self, trigger: &Event) -> Result<String> {
        let episode_id = Uuid::new_v4().to_string();
        let tensor = MaintenanceTensor::from_current_state();

        // Create active episode record
        let active = ActiveEpisode {
            episode_id: episode_id.clone(),
            trigger_event: trigger.event_type.clone(),
            start_timestamp: Utc::now(),
            services_involved: trigger.affected_services.clone(),
            pathways_activated: self.get_active_pathways().await?,
            events: vec![],
            tensor_signature: tensor,
        };

        // Store in active episodes map
        self.active_episodes.insert(episode_id.clone(), active.clone());

        // Insert initial record
        sqlx::query!(
            r#"
            INSERT INTO episodes (
                episode_id, start_timestamp, trigger_event, trigger_event_id,
                outcome, services_involved, pathways_activated, tensor_signature
            ) VALUES (?, ?, ?, ?, 'ongoing', ?, ?, ?)
            "#,
            episode_id,
            active.start_timestamp,
            trigger.event_type,
            trigger.event_id,
            serde_json::to_string(&active.services_involved)?,
            serde_json::to_string(&active.pathways_activated)?,
            tensor.to_bytes().as_slice()
        )
        .execute(&self.db)
        .await?;

        Ok(episode_id)
    }

    /// Add an event to an active episode
    pub async fn record_event(
        &self,
        episode_id: &str,
        event: &Event,
    ) -> Result<()> {
        if let Some(mut active) = self.active_episodes.get_mut(episode_id) {
            let sequence = active.events.len() as i32;
            let tensor = MaintenanceTensor::from_current_state();

            // Record event
            sqlx::query!(
                r#"
                INSERT INTO episode_events (
                    id, episode_id, event_type, event_data,
                    tensor_snapshot, sequence_number, timestamp
                ) VALUES (?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
                "#,
                Uuid::new_v4().to_string(),
                episode_id,
                event.event_type,
                serde_json::to_string(&event.data)?,
                tensor.to_bytes().as_slice(),
                sequence
            )
            .execute(&self.db)
            .await?;

            // Update active episode
            active.events.push(EpisodeEvent {
                event_type: event.event_type.clone(),
                timestamp: Utc::now(),
                tensor: tensor,
            });
        }

        Ok(())
    }

    /// Close an episode with resolution
    pub async fn close_episode(
        &self,
        episode_id: &str,
        resolution: &Resolution,
    ) -> Result<()> {
        let tensor_end = MaintenanceTensor::from_current_state();

        // Generate narrative
        let narrative = self.generate_narrative(episode_id).await?;

        // Calculate emotional valence based on outcome
        let valence = match resolution.outcome {
            Outcome::Success => 0.8,
            Outcome::Partial => 0.5,
            Outcome::Failure => 0.2,
            _ => 0.5,
        };

        // Extract lessons learned
        let lessons = self.extract_lessons(episode_id, resolution).await?;

        // Calculate pathway deltas
        let pathway_deltas = self.calculate_pathway_deltas(episode_id).await?;

        // Update episode record
        sqlx::query!(
            r#"
            UPDATE episodes
            SET
                end_timestamp = CURRENT_TIMESTAMP,
                resolution_event = ?,
                resolution_event_id = ?,
                outcome = ?,
                tensor_signature_end = ?,
                narrative = ?,
                emotional_valence = ?,
                lessons_learned = ?,
                pathway_deltas = ?,
                updated_at = CURRENT_TIMESTAMP
            WHERE episode_id = ?
            "#,
            resolution.event_type,
            resolution.event_id,
            resolution.outcome.to_string(),
            tensor_end.to_bytes().as_slice(),
            narrative,
            valence,
            serde_json::to_string(&lessons)?,
            serde_json::to_string(&pathway_deltas)?,
            episode_id
        )
        .execute(&self.db)
        .await?;

        // Remove from active episodes
        self.active_episodes.remove(episode_id);

        // Find and create links to similar episodes
        self.create_similarity_links(episode_id).await?;

        Ok(())
    }
}
```

### 3.2 Tensor Signature Recording

```rust
impl EpisodicMemory {
    /// Record episode with full tensor signature
    async fn record_episode_tensor(&self, episode: &Episode) -> Result<()> {
        let tensor = MaintenanceTensor::new(&ServiceState {
            id: episode.primary_service.clone(),
            health_score: episode.initial_health,
            synergy_score: episode.initial_synergy,
            // ... other state fields
        });

        sqlx::query!(
            r#"
            UPDATE episodes
            SET tensor_signature = ?
            WHERE episode_id = ?
            "#,
            tensor.to_bytes().as_slice(),
            episode.episode_id
        )
        .execute(&self.db)
        .await?;

        Ok(())
    }
}
```

---

## 4. Episode Links for Causal Chains

### 4.1 Link Creation

```rust
impl EpisodicMemory {
    /// Create links between episodes
    pub async fn create_episode_link(
        &self,
        source_id: &str,
        target_id: &str,
        link_type: LinkType,
        evidence: &str,
    ) -> Result<()> {
        // Calculate link strength based on evidence
        let strength = self.calculate_link_strength(source_id, target_id, &link_type).await?;

        sqlx::query!(
            r#"
            INSERT INTO episode_links (
                id, source_episode_id, target_episode_id,
                link_type, strength, evidence
            ) VALUES (?, ?, ?, ?, ?, ?)
            ON CONFLICT(source_episode_id, target_episode_id) DO UPDATE SET
                strength = ?,
                evidence = ?
            "#,
            Uuid::new_v4().to_string(),
            source_id,
            target_id,
            link_type.to_string(),
            strength,
            evidence,
            strength,
            evidence
        )
        .execute(&self.db)
        .await?;

        Ok(())
    }

    /// Automatically create similarity links when episode closes
    async fn create_similarity_links(&self, episode_id: &str) -> Result<()> {
        // Get episode tensor
        let episode = self.get_episode(episode_id).await?;
        let tensor = MaintenanceTensor::from_bytes(&episode.tensor_signature);

        // Find similar episodes
        let similar = self.find_similar_episodes(&tensor, 5).await?;

        for similar_episode in similar {
            if similar_episode.episode_id == episode_id {
                continue;
            }

            // Determine link type based on outcomes
            let link_type = if episode.outcome == similar_episode.outcome {
                LinkType::SimilarTo
            } else {
                LinkType::OppositeOf
            };

            self.create_episode_link(
                episode_id,
                &similar_episode.episode_id,
                link_type,
                &format!("Tensor similarity: {:.3}", similar_episode.similarity),
            ).await?;
        }

        Ok(())
    }

    /// Calculate link strength based on episode relationships
    async fn calculate_link_strength(
        &self,
        source_id: &str,
        target_id: &str,
        link_type: &LinkType,
    ) -> Result<f64> {
        let source = self.get_episode(source_id).await?;
        let target = self.get_episode(target_id).await?;

        let source_tensor = MaintenanceTensor::from_bytes(&source.tensor_signature);
        let target_tensor = MaintenanceTensor::from_bytes(&target.tensor_signature);

        // Base strength on tensor similarity
        let tensor_similarity = source_tensor.cosine_similarity(&target_tensor);

        // Adjust for link type
        let type_factor = match link_type {
            LinkType::Caused => 0.9,     // Strong causal claims
            LinkType::Prevented => 0.8,
            LinkType::SimilarTo => 0.7,
            LinkType::OppositeOf => 0.6,
            LinkType::Preceded => 0.5,
            LinkType::EscalatedTo => 0.85,
        };

        // Adjust for temporal proximity
        let time_diff = (target.start_timestamp - source.end_timestamp.unwrap_or(source.start_timestamp))
            .num_seconds().abs() as f64;
        let time_factor = 1.0 / (1.0 + time_diff / 3600.0); // Decay over hours

        Ok((tensor_similarity * type_factor * time_factor).clamp(0.0, 1.0))
    }
}
```

---

## 5. Similarity Recall

### 5.1 Recall View

```sql
-- View for finding relevant past episodes
CREATE VIEW v_relevant_episodes AS
SELECT
    e.episode_id,
    e.trigger_event,
    e.outcome,
    e.narrative,
    e.emotional_valence,
    e.lessons_learned,
    e.start_timestamp,
    e.end_timestamp,
    e.services_involved,
    e.tensor_signature,
    -- Note: actual similarity calculation done in application code
    0.0 as relevance_score  -- Placeholder
FROM episodes e
WHERE e.outcome IS NOT NULL
AND e.outcome != 'ongoing'
ORDER BY e.start_timestamp DESC
LIMIT 100;
```

### 5.2 Similarity Recall Implementation

```rust
impl EpisodicMemory {
    /// Recall similar past episodes based on current state
    pub async fn recall_similar(
        &self,
        current_tensor: &MaintenanceTensor,
        limit: usize,
    ) -> Result<Vec<SimilarEpisode>> {
        // Get all completed episodes with tensors
        let episodes: Vec<Episode> = sqlx::query_as!(
            Episode,
            r#"
            SELECT * FROM episodes
            WHERE outcome IS NOT NULL
            AND outcome != 'ongoing'
            AND tensor_signature IS NOT NULL
            ORDER BY start_timestamp DESC
            LIMIT 500
            "#
        )
        .fetch_all(&self.db)
        .await?;

        // Calculate similarity for each
        let mut scored: Vec<SimilarEpisode> = episodes.iter()
            .filter_map(|e| {
                let tensor = MaintenanceTensor::from_bytes(
                    &e.tensor_signature.as_ref()?
                        .try_into().ok()?
                );
                let similarity = current_tensor.cosine_similarity(&tensor);

                Some(SimilarEpisode {
                    episode: e.clone(),
                    similarity,
                })
            })
            .collect();

        // Sort by similarity descending
        scored.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap());

        // Return top N
        Ok(scored.into_iter().take(limit).collect())
    }

    /// Recall episodes for a specific trigger type
    pub async fn recall_by_trigger(
        &self,
        trigger_event: &str,
        limit: usize,
    ) -> Result<Vec<Episode>> {
        sqlx::query_as!(
            Episode,
            r#"
            SELECT * FROM episodes
            WHERE trigger_event = ?
            AND outcome IS NOT NULL
            ORDER BY emotional_valence DESC, start_timestamp DESC
            LIMIT ?
            "#,
            trigger_event,
            limit as i32
        )
        .fetch_all(&self.db)
        .await
    }

    /// Get causal chain leading to an episode
    pub async fn get_causal_chain(&self, episode_id: &str) -> Result<Vec<Episode>> {
        let mut chain = Vec::new();
        let mut current_id = episode_id.to_string();

        // Walk backwards through causal links
        for _ in 0..10 {  // Max depth 10
            let link = sqlx::query!(
                r#"
                SELECT source_episode_id
                FROM episode_links
                WHERE target_episode_id = ?
                AND link_type = 'caused'
                ORDER BY strength DESC
                LIMIT 1
                "#,
                current_id
            )
            .fetch_optional(&self.db)
            .await?;

            match link {
                Some(l) => {
                    let episode = self.get_episode(&l.source_episode_id).await?;
                    chain.push(episode);
                    current_id = l.source_episode_id;
                }
                None => break,
            }
        }

        chain.reverse();  // Oldest first
        Ok(chain)
    }
}
```

---

## 6. Integration with Remediation

### 6.1 Remediation Check

```rust
impl L0Remediation {
    /// Check similar past episodes before acting
    async fn check_similar_episodes(&self, action: &Action) -> Result<EpisodeCheck> {
        let current_tensor = MaintenanceTensor::from_current_state();
        let similar = self.episodic_memory
            .recall_similar(&current_tensor, 5)
            .await?;

        // Check for failed similar episodes
        let failures: Vec<_> = similar.iter()
            .filter(|e| e.episode.outcome == Some("failure".to_string()))
            .filter(|e| e.similarity > 0.8)  // High similarity threshold
            .collect();

        if !failures.is_empty() {
            return Ok(EpisodeCheck::Warning {
                message: format!(
                    "Similar action failed {} time(s) in past. Most recent: {}",
                    failures.len(),
                    failures[0].episode.narrative.as_deref().unwrap_or("Unknown")
                ),
                recommended: "Consider alternative approach or escalate to L1",
            });
        }

        // Check for successful similar episodes
        let successes: Vec<_> = similar.iter()
            .filter(|e| e.episode.outcome == Some("success".to_string()))
            .filter(|e| e.similarity > 0.7)
            .collect();

        if !successes.is_empty() {
            return Ok(EpisodeCheck::Confidence {
                boost: 0.05 * successes.len() as f64,
                reason: format!("Similar successful episodes: {}", successes.len()),
            });
        }

        Ok(EpisodeCheck::NoMatch)
    }
}
```

---

## 7. Acceptance Criteria

- [ ] episodic_memory.db schema created
- [ ] Episodes recorded with 12D tensor signatures
- [ ] Episode links capture causal relationships
- [ ] v_relevant_episodes view enables similarity recall
- [ ] Remediation checks similar past episodes before acting
- [ ] Episode narrative generation implemented

---

## 8. References

- **Hebbian Integration:** `nam/HEBBIAN_INTEGRATION.md`
- **Tensor Encoding:** `nam/TENSOR_ENCODING.md`
- **Self-Model:** `nam/SELF_MODEL.md`
- **NAM Gap Analysis:** `NAM_GAP_ANALYSIS_REPORT.md`

---

*Document generated for NAM Phase 5 compliance*
*Episodic Memory: Where the system remembers what happened when*
