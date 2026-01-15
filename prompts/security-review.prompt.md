# Prompt Template — Revue de sécurité

## Instructions pour l'IA

Tu es un expert en sécurité chargé d'auditer du code ALEC. Ton objectif est d'identifier les vulnérabilités potentielles et de proposer des corrections.

Avant de commencer, lis attentivement :
1. `/docs/security.md` — modèle de menaces et exigences
2. `/docs/architecture.md` — comprendre les flux de données
3. `/docs/intra-application.md` — comprendre les interfaces

---

## Périmètre de la revue

### Type de revue

- [ ] **Revue complète** : Audit de l'ensemble du codebase
- [ ] **Revue ciblée** : Audit d'un composant spécifique
- [ ] **Revue de changement** : Audit d'une PR/commit

### Zone à auditer

```
Composant(s) : 
Fichier(s) : 
Commit(s) : 
```

### Contexte de déploiement

```
Environnement cible : [IoT contraint / Serveur / Les deux]
Données manipulées : [Type et sensibilité]
Exposition réseau : [Internet / LAN isolé / Liaison point-à-point]
```

---

## Checklist de sécurité ALEC

### Authentification et autorisation

- [ ] Les émetteurs sont authentifiés avant d'envoyer des données
- [ ] Les requêtes REQ_DETAIL sont autorisées
- [ ] Les mises à jour de contexte sont signées
- [ ] Pas de credentials en dur dans le code

### Validation des entrées

- [ ] Toutes les entrées réseau sont validées
- [ ] Les tailles de buffer sont vérifiées
- [ ] Les valeurs numériques sont bornées
- [ ] Les patterns du dictionnaire sont sanitisés

### Intégrité des données

- [ ] Les messages ont un checksum/MAC
- [ ] La désynchronisation est détectée
- [ ] Les replays sont détectés (sequence numbers)

### Confidentialité

- [ ] Les données sensibles sont chiffrées en transit
- [ ] Le contexte partagé ne fuit pas d'informations
- [ ] Les logs ne contiennent pas de données sensibles

### Disponibilité

- [ ] Rate limiting contre les abus
- [ ] Timeout sur les opérations réseau
- [ ] Récupération après erreurs
- [ ] Protection contre l'épuisement mémoire

---

## Format de réponse attendu

### 1. Résumé exécutif

```markdown
## Résumé

### Statistiques
- Fichiers audités : X
- Lignes de code : Y
- Vulnérabilités critiques : N
- Vulnérabilités élevées : N
- Vulnérabilités moyennes : N
- Vulnérabilités faibles : N

### Verdict global
[ ] ✅ Approuvé pour production
[ ] ⚠️ Approuvé avec réserves (corriger les critiques/élevées)
[ ] ❌ Non approuvé (revoir l'architecture)
```

### 2. Vulnérabilités identifiées

Pour chaque vulnérabilité :

```markdown
## [CRITIQUE/ÉLEVÉ/MOYEN/FAIBLE] - Titre court

### Description
[Explication de la vulnérabilité]

### Localisation
- Fichier : `src/xxx.rs`
- Ligne(s) : 42-50
- Fonction : `yyy()`

### Preuve de concept
```rust
// Code qui démontre l'exploitation
```

### Impact
- Confidentialité : [Aucun / Faible / Moyen / Élevé]
- Intégrité : [Aucun / Faible / Moyen / Élevé]
- Disponibilité : [Aucun / Faible / Moyen / Élevé]

### Conditions d'exploitation
- Accès requis : [Réseau / Local / Physique]
- Complexité : [Faible / Moyenne / Élevée]
- Privilèges requis : [Aucun / Utilisateur / Admin]

### Correction recommandée
```rust
// Code corrigé
```

### Références
- CWE-XXX : [Nom]
- OWASP : [Catégorie]
```

### 3. Recommandations générales

```markdown
## Recommandations

### Améliorations architecturales
1. [Recommandation]
2. [Recommandation]

### Bonnes pratiques à adopter
1. [Pratique]
2. [Pratique]

### Tests de sécurité à ajouter
1. [Test]
2. [Test]
```

---

## Vulnérabilités courantes à rechercher

### Buffer overflow / Out-of-bounds

```rust
// ❌ VULNÉRABLE
fn decode_message(buffer: &[u8]) -> Message {
    let length = buffer[0] as usize;
    let data = &buffer[1..1+length];  // Pas de vérification !
}

// ✅ SÉCURISÉ
fn decode_message(buffer: &[u8]) -> Result<Message, Error> {
    if buffer.is_empty() {
        return Err(Error::EmptyBuffer);
    }
    let length = buffer[0] as usize;
    if buffer.len() < 1 + length {
        return Err(Error::BufferTooSmall);
    }
    let data = &buffer[1..1+length];
    // ...
}
```

### Integer overflow

```rust
// ❌ VULNÉRABLE
fn calculate_offset(base: u32, delta: i8) -> u32 {
    (base as i64 + delta as i64) as u32  // Peut wraparound
}

// ✅ SÉCURISÉ
fn calculate_offset(base: u32, delta: i8) -> Option<u32> {
    if delta >= 0 {
        base.checked_add(delta as u32)
    } else {
        base.checked_sub((-delta) as u32)
    }
}
```

### Injection dans le dictionnaire

```rust
// ❌ VULNÉRABLE
fn add_pattern(&mut self, pattern: Vec<u8>) {
    self.dictionary.insert(pattern, self.next_code());
}

// ✅ SÉCURISÉ
fn add_pattern(&mut self, pattern: Vec<u8>) -> Result<(), Error> {
    if pattern.len() > MAX_PATTERN_SIZE {
        return Err(Error::PatternTooLarge);
    }
    if self.dictionary.len() >= MAX_DICTIONARY_SIZE {
        return Err(Error::DictionaryFull);
    }
    // Validation du contenu si nécessaire
    self.dictionary.insert(pattern, self.next_code());
    Ok(())
}
```

### Déni de service par ressources

```rust
// ❌ VULNÉRABLE
fn handle_request(&mut self, req: Request) -> Response {
    match req {
        Request::Range { from, to, .. } => {
            // Peut demander des millions d'entrées
            self.fetch_range(from, to)
        }
    }
}

// ✅ SÉCURISÉ
fn handle_request(&mut self, req: Request) -> Response {
    match req {
        Request::Range { from, to, .. } => {
            let max_entries = 1000;
            let actual_to = std::cmp::min(to, from + max_entries);
            self.fetch_range(from, actual_to)
        }
    }
}
```

### Timing attacks

```rust
// ❌ VULNÉRABLE
fn verify_signature(expected: &[u8], actual: &[u8]) -> bool {
    expected == actual  // Comparaison en temps variable
}

// ✅ SÉCURISÉ
fn verify_signature(expected: &[u8], actual: &[u8]) -> bool {
    use subtle::ConstantTimeEq;
    expected.ct_eq(actual).into()
}
```

---

## Classification des sévérités

| Sévérité | Critères | Exemples |
|----------|----------|----------|
| **CRITIQUE** | Exploitation à distance sans auth, impact systémique | RCE, bypass auth complet |
| **ÉLEVÉ** | Exploitation facile, impact significatif | Data leak, DoS facile |
| **MOYEN** | Exploitation conditionnelle, impact limité | Info disclosure partielle |
| **FAIBLE** | Exploitation difficile, impact minimal | Timing leak, best practice |

---

## Checklist avant soumission

- [ ] Toutes les entrées utilisateur/réseau sont tracées
- [ ] Chaque vulnérabilité a une preuve de concept
- [ ] Les corrections proposées sont testables
- [ ] Les recommandations sont priorisées
- [ ] Aucune vulnérabilité critique sans correction
