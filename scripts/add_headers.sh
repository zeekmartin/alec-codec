#!/bin/bash

HEADER='// ALEC - Adaptive Lazy Evolving Compression
// Copyright (c) 2025 David Martin Venti
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.

'

# Trouver tous les fichiers .rs dans src/
for file in $(find src -name "*.rs"); do
    # Vérifier si le header existe déjà
    if ! grep -q "ALEC - Adaptive" "$file"; then
        echo "Adding header to $file"
        # Créer fichier temporaire avec header + contenu original
        echo "$HEADER" | cat - "$file" > temp && mv temp "$file"
    else
        echo "Header already exists in $file"
    fi
done

echo "Done!"
