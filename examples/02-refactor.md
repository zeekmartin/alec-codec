# Exemple — Refactoring

## Contexte

Cet exemple montre comment refactoriser du code ALEC en préservant le comportement existant.

**Cible** : Simplifier la fonction `classify()` qui est devenue trop complexe.

---

## Étape 1 : Demande de refactoring

```
REFACTORING: Simplifier la fonction classify()

Zone : src/classifier.rs, lignes 45-145
Type : Simplification + Extraction

Problème actuel : 
- Fonction de 100 lignes
- Complexité cyclomatique de 18
- 7 niveaux d'indentation max
- Difficile à tester unitairement

Objectif :
- Maximum 25 lignes par fonction
- Complexité < 8
- Maximum 3 niveaux d'indentation
- Fonctions auxiliaires testables individuellement

Invariants :
- [x] API publique inchangée
- [x] Même classification pour mêmes entrées
```

---

## Étape 2 : Code original (avant)

```rust
// src/classifier.rs - AVANT REFACTORING

impl Classifier {
    pub fn classify(&self, data: &RawData, context: &Context) -> Classification {
        let prediction = context.predict(data.source_id);
        
        if let Some(pred) = prediction {
            let delta = (data.value - pred.value).abs();
            let relative_delta = if pred.value != 0.0 {
                delta / pred.value.abs()
            } else {
                delta
            };
            
            // Vérifier les seuils critiques
            if let Some(critical_thresholds) = self.config.critical_thresholds.get(&data.source_id) {
                if data.value < critical_thresholds.min {
                    return Classification {
                        priority: Priority::P1Critical,
                        reason: ClassificationReason::ThresholdExceeded {
                            threshold: critical_thresholds.min,
                            actual: data.value,
                        },
                        delta: relative_delta,
                        confidence: 1.0,
                    };
                }
                if data.value > critical_thresholds.max {
                    return Classification {
                        priority: Priority::P1Critical,
                        reason: ClassificationReason::ThresholdExceeded {
                            threshold: critical_thresholds.max,
                            actual: data.value,
                        },
                        delta: relative_delta,
                        confidence: 1.0,
                    };
                }
            }
            
            // Vérifier les anomalies statistiques
            if relative_delta > self.config.anomaly_threshold {
                if relative_delta > self.config.critical_anomaly_threshold {
                    return Classification {
                        priority: Priority::P1Critical,
                        reason: ClassificationReason::AnomalyDetected {
                            anomaly_type: AnomalyType::ExtremeDeviation,
                        },
                        delta: relative_delta,
                        confidence: pred.confidence,
                    };
                } else {
                    return Classification {
                        priority: Priority::P2Important,
                        reason: ClassificationReason::AnomalyDetected {
                            anomaly_type: AnomalyType::SignificantDeviation,
                        },
                        delta: relative_delta,
                        confidence: pred.confidence,
                    };
                }
            }
            
            // Vérifier si c'est une transmission programmée
            if self.is_scheduled_transmission(data.timestamp) {
                return Classification {
                    priority: Priority::P3Normal,
                    reason: ClassificationReason::ScheduledTransmission,
                    delta: relative_delta,
                    confidence: pred.confidence,
                };
            }
            
            // Valeur normale, peut être différée
            if relative_delta < self.config.minimum_delta_threshold {
                return Classification {
                    priority: Priority::P5Disposable,
                    reason: ClassificationReason::BelowMinimumDelta,
                    delta: relative_delta,
                    confidence: pred.confidence,
                };
            }
            
            Classification {
                priority: Priority::P4Deferred,
                reason: ClassificationReason::NormalValue,
                delta: relative_delta,
                confidence: pred.confidence,
            }
        } else {
            // Pas de prédiction disponible, envoyer comme normal
            Classification {
                priority: Priority::P3Normal,
                reason: ClassificationReason::NoPrediction,
                delta: 0.0,
                confidence: 0.0,
            }
        }
    }
}
```

---

## Étape 3 : Diagnostic

### Code smells identifiés

1. **Fonction trop longue** (100 lignes)
2. **Niveaux d'imbrication excessifs** (7 niveaux)
3. **Logique mélangée** (seuils, anomalies, scheduling)
4. **Répétition** (construction de Classification)
5. **Difficile à tester** (pas de points d'entrée intermédiaires)

### Métriques

| Métrique | Avant | Objectif |
|----------|-------|----------|
| Lignes | 100 | < 25 par fonction |
| Complexité cyclomatique | 18 | < 8 |
| Niveaux d'indentation | 7 | < 4 |
| Fonctions auxiliaires | 0 | 4-5 |

---

## Étape 4 : Plan de refactoring

### Étape 4.1 : Extraire le calcul du delta

```rust
fn calculate_delta(value: f64, prediction: &Prediction) -> DeltaInfo {
    let absolute = (value - prediction.value).abs();
    let relative = if prediction.value != 0.0 {
        absolute / prediction.value.abs()
    } else {
        absolute
    };
    DeltaInfo { absolute, relative }
}
```

### Étape 4.2 : Extraire la vérification des seuils critiques

```rust
fn check_critical_thresholds(
    &self,
    value: f64,
    source_id: u32,
    delta: &DeltaInfo,
) -> Option<Classification> {
    // ...
}
```

### Étape 4.3 : Extraire la détection d'anomalies

```rust
fn check_anomaly(
    &self,
    delta: &DeltaInfo,
    confidence: f32,
) -> Option<Classification> {
    // ...
}
```

### Étape 4.4 : Extraire la classification normale

```rust
fn classify_normal(
    &self,
    timestamp: u64,
    delta: &DeltaInfo,
    confidence: f32,
) -> Classification {
    // ...
}
```

---

## Étape 5 : Code refactorisé (après)

```rust
// src/classifier.rs - APRÈS REFACTORING

/// Informations sur l'écart entre valeur et prédiction
struct DeltaInfo {
    absolute: f64,
    relative: f64,
}

impl Classifier {
    /// Point d'entrée principal - délègue aux fonctions spécialisées
    pub fn classify(&self, data: &RawData, context: &Context) -> Classification {
        match context.predict(data.source_id) {
            Some(prediction) => self.classify_with_prediction(data, &prediction),
            None => Classification::no_prediction(),
        }
    }
    
    fn classify_with_prediction(
        &self,
        data: &RawData,
        prediction: &Prediction,
    ) -> Classification {
        let delta = Self::calculate_delta(data.value, prediction);
        
        // Chaîne de responsabilité : première classification qui matche
        self.check_critical_thresholds(data.value, data.source_id, &delta)
            .or_else(|| self.check_anomaly(&delta, prediction.confidence))
            .unwrap_or_else(|| self.classify_normal(data.timestamp, &delta, prediction.confidence))
    }
    
    /// Calcule l'écart absolu et relatif
    fn calculate_delta(value: f64, prediction: &Prediction) -> DeltaInfo {
        let absolute = (value - prediction.value).abs();
        let relative = if prediction.value.abs() > f64::EPSILON {
            absolute / prediction.value.abs()
        } else {
            absolute
        };
        DeltaInfo { absolute, relative }
    }
    
    /// Vérifie les seuils critiques (min/max absolus)
    fn check_critical_thresholds(
        &self,
        value: f64,
        source_id: u32,
        delta: &DeltaInfo,
    ) -> Option<Classification> {
        let thresholds = self.config.critical_thresholds.get(&source_id)?;
        
        let violated_threshold = if value < thresholds.min {
            Some(thresholds.min)
        } else if value > thresholds.max {
            Some(thresholds.max)
        } else {
            None
        }?;
        
        Some(Classification::critical_threshold(violated_threshold, value, delta.relative))
    }
    
    /// Détecte les anomalies statistiques
    fn check_anomaly(&self, delta: &DeltaInfo, confidence: f32) -> Option<Classification> {
        if delta.relative <= self.config.anomaly_threshold {
            return None;
        }
        
        let (priority, anomaly_type) = if delta.relative > self.config.critical_anomaly_threshold {
            (Priority::P1Critical, AnomalyType::ExtremeDeviation)
        } else {
            (Priority::P2Important, AnomalyType::SignificantDeviation)
        };
        
        Some(Classification::anomaly(priority, anomaly_type, delta.relative, confidence))
    }
    
    /// Classifie une valeur normale (pas d'alerte)
    fn classify_normal(&self, timestamp: u64, delta: &DeltaInfo, confidence: f32) -> Classification {
        if self.is_scheduled_transmission(timestamp) {
            Classification::scheduled(delta.relative, confidence)
        } else if delta.relative < self.config.minimum_delta_threshold {
            Classification::disposable(delta.relative, confidence)
        } else {
            Classification::deferred(delta.relative, confidence)
        }
    }
}

// Constructeurs pour Classification (builder pattern simplifié)
impl Classification {
    fn no_prediction() -> Self {
        Self {
            priority: Priority::P3Normal,
            reason: ClassificationReason::NoPrediction,
            delta: 0.0,
            confidence: 0.0,
        }
    }
    
    fn critical_threshold(threshold: f64, actual: f64, delta: f64) -> Self {
        Self {
            priority: Priority::P1Critical,
            reason: ClassificationReason::ThresholdExceeded { threshold, actual },
            delta,
            confidence: 1.0,
        }
    }
    
    fn anomaly(priority: Priority, anomaly_type: AnomalyType, delta: f64, confidence: f32) -> Self {
        Self {
            priority,
            reason: ClassificationReason::AnomalyDetected { anomaly_type },
            delta,
            confidence,
        }
    }
    
    fn scheduled(delta: f64, confidence: f32) -> Self {
        Self {
            priority: Priority::P3Normal,
            reason: ClassificationReason::ScheduledTransmission,
            delta,
            confidence,
        }
    }
    
    fn disposable(delta: f64, confidence: f32) -> Self {
        Self {
            priority: Priority::P5Disposable,
            reason: ClassificationReason::BelowMinimumDelta,
            delta,
            confidence,
        }
    }
    
    fn deferred(delta: f64, confidence: f32) -> Self {
        Self {
            priority: Priority::P4Deferred,
            reason: ClassificationReason::NormalValue,
            delta,
            confidence,
        }
    }
}
```

---

## Étape 6 : Validation

### Tests existants

```bash
$ cargo test classifier
running 12 tests
test classifier::test_classify_no_prediction ... ok
test classifier::test_classify_critical_min ... ok
test classifier::test_classify_critical_max ... ok
test classifier::test_classify_extreme_anomaly ... ok
test classifier::test_classify_significant_anomaly ... ok
test classifier::test_classify_scheduled ... ok
test classifier::test_classify_disposable ... ok
test classifier::test_classify_deferred ... ok
test classifier::test_classify_edge_at_threshold ... ok
test classifier::test_classify_zero_prediction ... ok
test classifier::test_classify_negative_values ... ok
test classifier::test_classify_integration ... ok

test result: ok. 12 passed; 0 failed
```

### Nouveaux tests unitaires (pour les fonctions extraites)

```rust
#[test]
fn test_calculate_delta_normal() {
    let prediction = Prediction { value: 100.0, confidence: 0.9 };
    let delta = Classifier::calculate_delta(110.0, &prediction);
    
    assert!((delta.absolute - 10.0).abs() < 0.001);
    assert!((delta.relative - 0.1).abs() < 0.001);
}

#[test]
fn test_calculate_delta_zero_prediction() {
    let prediction = Prediction { value: 0.0, confidence: 0.9 };
    let delta = Classifier::calculate_delta(5.0, &prediction);
    
    assert!((delta.absolute - 5.0).abs() < 0.001);
    assert!((delta.relative - 5.0).abs() < 0.001);  // Fallback to absolute
}

#[test]
fn test_check_critical_thresholds_below_min() {
    let classifier = Classifier::with_thresholds(0.0, 100.0);
    let delta = DeltaInfo { absolute: 5.0, relative: 0.1 };
    
    let result = classifier.check_critical_thresholds(-5.0, 1, &delta);
    
    assert!(result.is_some());
    assert_eq!(result.unwrap().priority, Priority::P1Critical);
}
```

### Métriques après refactoring

| Métrique | Avant | Après | Objectif |
|----------|-------|-------|----------|
| Lignes (fonction principale) | 100 | 8 | ✅ < 25 |
| Complexité cyclomatique | 18 | 6 | ✅ < 8 |
| Niveaux d'indentation max | 7 | 2 | ✅ < 4 |
| Fonctions | 1 | 6 | ✅ Testables |

---

## Leçons apprises

1. **Extraire tôt** : Dès qu'une fonction dépasse 30 lignes, envisager l'extraction
2. **Nommer clairement** : `check_critical_thresholds` est plus clair que le code inline
3. **Chaîne de responsabilité** : `or_else` simplifie la logique de fallback
4. **Builder pattern** : Les constructeurs nommés (`Classification::anomaly`) documentent l'intention
5. **Tests préservés** : Le refactoring n'a cassé aucun test existant
