# Règles de nettoyage des crédits

Document interne. Il ne part **jamais** en amont : les PR upstream ne contiennent
que `sources/en.chikari/` et `sources/en.novelbuddy/`, jamais ce dossier `docs/`.

Il décrit ce que fait `watermark.rs` et surtout **pourquoi**, avec les mesures qui
ont tranché chaque choix. Les chiffres viennent d'un corpus de 127 chapitres :
94 chapitres de quatre romans plus les chapitres 41 à 51 de quatre autres.

## Le fichier

| | |
|:--|:--|
| Code | `sources/en.chikari/src/watermark.rs` |
| Copie | `sources/en.novelbuddy/src/watermark.rs`, **identique octet pour octet** |
| Idem pour | `markdown.rs` et `mtl.rs` |
| Appelé par | `markdown.rs`, une fois **par paragraphe**, si le réglage `hide_watermark` est actif |
| Taille | 519 lignes |

Toute modification se fait dans une copie puis se recopie dans l'autre. Vérifier
après : `md5sum` des deux doit être égal.

## Le modèle

Trois définitions portent tout le reste.

**Prose** : trois blocs, plus les blancs de n'importe où.

| Bloc | Plage |
|:--|:--|
| Basic Latin | `0x0000`-`0x007F` |
| Latin-1 Supplement | `0x0080`-`0x00FF` |
| General Punctuation | `0x2000`-`0x206F` |

Les blancs Unicode s'ajoutent (`is_whitespace`), parce qu'une espace idéographique
n'est pas une marque du crédit mais un trou entre deux mots.

**Signe** : ni prose, ni alphanumérique. Un crochet CJK, un tiret cadratin exotique,
une puce.

**Étranger** : hors prose **et** alphanumérique. Une lettre ou un chiffre d'un autre
script, ce avec quoi le crédit s'écrit.

Attention : `*`, `°`, `-`, `/`, `{`, `}` sont **dans** les blocs de prose. Ils ne sont
donc jamais des signes au sens strict, mais la règle 1 les consomme quand même parce
que son test est << ni alphanumérique, ni blanc >>.

## Les règles, dans l'ordre

### 1. Le crédit épelé - `strip_injected`

**Détection** : une lettre, puis une alternance séparateur + lettre, **5 lettres
minimum**, en partant d'une frontière de mot.

**Retrait des marques** : de chaque côté, **indépendamment**. On saute les espaces
collés à l'injection, puis on consomme la suite de caractères ni alphanumériques ni
blancs, et on s'arrête.

**Recollage** : l'espace laissé est ramené à 1, ou au multiple de 4 inférieur, pour
que l'indentation garde son cran.

| Entrée réelle | Sortie |
|:--|:--|
| `"I /N_o_v_e_l_i_g_h_t/ heard there is a village nearby."` | `"I heard there is a village nearby."` |
| `Someone then smacked me {N*o*v*e*l*i*g*h*t} on the back.` | `Someone then smacked me on the back.` |
| `I was wondering what to °* N v l i g h t *° do if she ran away.` | `I was wondering what to do if she ran away.` |

**Écarté** : passer le seuil à 4 lettres. Mesuré 0 chapitre de différence, et cela
retirerait un vrai sigle écrit avec séparateurs (`N.A.S.A.`).

**Écarté** : consommer jusqu'au premier alphanumérique, blancs compris. Mesuré 3
chapitres et **mange de la ponctuation** : `little,` > `little`, `that.` > `that`,
`workers'` > `workers`.

### 2. Le marqueur de dialogue - `dialogue_line`

Le **lead** est la suite de tête faite de signes, de guillemets et d'espaces.

| Cas | Action |
|:--|:--|
| le lead n'a pas à la fois un guillemet et un signe | `dialogue_marks` |
| le lead a les deux, et un fermant apparaît après le lead | on garde l'ouvrant |
| le lead a les deux, et aucun fermant n'apparaît | cadratin, et le fermant de paire orphelin part |

`dialogue_marks` : si la ligne finit par un signe, `wrapped_text` garde l'intérieur ;
sinon la suite de signes de tête devient un cadratin.

Le remplacement est **U+2014**, jamais `-` ASCII : en markdown, un `-` en début de
ligne crée une puce. Un tiret déjà présent n'est jamais touché, parce que `-`, `-` et
`-` sont dans les blocs de prose.

| Entrée réelle | Sortie |
|:--|:--|
| `\u{0F3A}\u{0F3A} Amelia Merigold (1) \u{0F3B}` | `Amelia Merigold (1)` |
| `"-Hello," she said.` (ferme au milieu) | `"Hello," she said.` |

**Décidé** : ne pas retirer un fermant mal apparié. 20 lignes du corpus ouvrent et
ferment sur des guillemets de familles différentes, et le fermant est chaque fois
légitime.

### 3. Les paires mortes - `without_dead_pairs`

**Les guillemets sont exclus** : une réplique citée est de la parole à garder, pas un
crochet à couper. `PAIRS` les garde pour `quote_close` et `matching_opener`, qui en
ont besoin pour apparier.

Pour les crochets, une pile apparie chaque ouvrante à sa fermante, types mélangés
compris, puis les paires sont décidées **de l'intérieur vers l'extérieur**.

| Contenu | Action |
|:--|:--|
| blanc | la paire part |
| hors prose, avec des phrases lisibles | on garde les phrases dont aucun mot n'est bruité |
| hors prose, aucune phrase lisible | la paire et son contenu partent |
| tout en prose | la paire reste |

| Entrée réelle | Sortie |
|:--|:--|
| `Edited by: faker[T/N: join the dşçöŕd (here) for spoilers, ...]` | `Edited by: faker` |
| `He said (a [\] c) then left.` | `He said (a c) then left.` |

L'intérieur d'abord change la sortie : juger l'extérieure en premier l'emportait avec
le crochet intérieur. Mesuré 0 chapitre de différence, mais la forme est meilleure.

**Sans cette exclusion des guillemets** : une réplique entière disparaissait dès
qu'un signe s'y trouvait. 18 lignes perdues sur 8 656, dont deux de vrai dialogue.

**Écarté** : retirer les guillemets de `PAIRS`. Mesuré 13 chapitres, +50 / -5 mots,
mais `quote_close` ne sait alors plus apparier, et un fermant reste orphelin.

### 4. La garde anticipée - `clean_line`

Si la ligne n'a **aucun** caractère hors prose **et aucun** domaine, elle est rendue
telle quelle. Sur un échantillon de 20 chapitres, **1 970 lignes** s'arrêtent ici.

### 5. Le découpage en phrases - `sentences`

Coupe sur `. ! ? ...` et les variantes doubles, avale les fermants accolés, **exige un
blanc** après. `Lv. 7 > Lv. 8` donne trois phrases ; `word.word` n'en donne qu'une.

### 6. La décision, phrase par phrase

| Ordre | Test | Action | Exemple réel |
|:-:|:--|:--|:--|
| 6a | domaine (un point, un caractère de mot avant, 2 ou 3 minuscules après) | supprime la phrase | `The source of this cntent is frewebnovel.com.` |
| 6b | prose pure | garde | `This is to make it cohesive.` |
| 6c | un crédit d'un seul tenant | voir ci-dessous | - |
| 6d | repli | garde les caractères de prose, s'il reste un mot | `{1} Explanation:  is Jeoksangari, ...` |
| 6e | rien de lisible | supprime | `.`, `*` |

**6c** : pour chaque mot, retirer les signes ; une suite de signes entre deux lettres
devient **une espace** ; jeter le mot s'il porte encore une lettre étrangère ; garder
le reste.

| Entrée réelle | Sortie |
|:--|:--|
| `... traces of \ \ her past through the Thieves' Guild?` | phrase entière |
| `talent\u{300A}Farsight\u{300B}being activated` | `talent Farsight being activated` |
| `Talents: \u{300A}Farsight\u{300B}\u{300A}Calm\u{300B} Thinking` | `Talents: Farsight Calm Thinking` |

Les crochets CJK U+300A / U+300B du corpus servent de **séparateurs de mots** : les
transformer en espace est le comportement correct.

**6d** reste utile : quand tous les mots sont étrangers, 6c ne rend rien et 6d sauve
les lettres de prose (`Fųck` > `Fck`). Mesuré : 6d intervient sur **8 phrases sur
102**, contre 46 avant la règle 6c actuelle.

### 7. Le recollage

Phrases par une espace, lignes par `\n`, fin rognée.

## Les listes

| Liste | Contenu | Sert à |
|:--|:--|:--|
| `PROSE_BLOCKS` | 3 blocs | décider ce qui est du texte |
| `PAIRS` | 11 paires | apparier et fermer une citation |
| `QUOTES` | 10 caractères | reconnaître de la parole |
| `SENTENCE_ENDS` | 9 caractères | couper les phrases |
| `SPELLED_MIN_LETTERS` | 5 | seuil de l'épellation |

`PAIRS` sert à **supprimer**, `QUOTES` seulement à **lire**. C'est pourquoi les
guillemets ASCII sont dans `QUOTES` et pas dans `PAIRS` : ils s'écrivent seuls
(pouces, apostrophe) et y mettre exposerait à supprimer du texte.

## Mesures de référence

Contre la v32 publiée, sur 127 chapitres :

| Mesure | Valeur |
|:--|--:|
| Chapitres changés | **20** |
| Mots perdus | **23** |
| Mots gagnés | **28** |
| Caractères hors prose en sortie | **0** |
| Lettres étrangères en sortie | **0** |

Les 23 mots perdus sont tous du crédit, des fragments de domaine mutilés, ou la forme
recollée `FarsightCalm` qui devient deux mots.

Les 28 gagnés sont de la prose réelle (`Farsight`, `Calm`, `erased`, `doves`,
`daring`, `encourage`, `absolute`, `oppose`) plus **trois débris** de crédit.

Chapitres 22 à 28 de `shadow-slave`, `city-of-witches` et `chaotic-craftsman` :
**21 identiques**. Les changements sont localisés sur les chapitres qui portent un
crédit.

## Limites connues

| Cas | Comportement | Statut |
|:--|:--|:--|
| Crédit entièrement en prose (`Nvlght`, `frewebnovel.com`) | laissé | accepté, aucune règle de vocabulaire |
| Trois débris entre parenthèses (`(Exclusive on)`, `(Read the full story)`, `(Don't copy, read here)`) | laissés | accepté, prix de la récupération des répliques |
| Phrase portant un domaine | supprimée entière | accepté |
| Crochets mélangés mal formés `([dead) useful]` | laissés intacts | accepté |

## Décisions écartées, avec leur mesure

| Idée | Mesure | Verdict |
|:--|:--|:--|
| Seuil d'épellation à 4 | 0 chapitre | écarté, risque de sigle |
| Consommer jusqu'à l'alphanumérique | 3 chapitres, -3 signes | écarté, mange la ponctuation |
| Guillemets hors de `PAIRS` | 13 chapitres, +50 / -5 | écarté, casse l'appariement |
| Retirer les guillemets mal appariés | 20 lignes concernées | écarté, ponctuation légitime |
| Passe intérieure explicite pour les crochets | 0 chapitre | appliqué quand même, plus juste |
| Logique des parenthèses pour la règle 6 | 29 chapitres, **-202 mots** | écarté, jette les phrases et les fiches |
| Parentèse courte jetée (option 2) | 46 chapitres, **-146 mots** | écarté, jette `(gambas)`, `(1)`, les incises |
| Crédits nettoyés avant les guillemets | 0 chapitre | neutre, ordre actuel conservé |
| Guillemets corrigés en dernier | 29 chapitres, **-86 mots** | écarté |

## Points ouverts

- Retirer le guillemet orphelin **en fin** de ligne : la variante est écrite, elle ne
  se déclenche sur aucun chapitre. Le `+2` du chapitre 41 de `city-of-witches` n'est
  pas un défaut mais un fermant légitime conservé.
- Les trois débris de crédit : seul un vocabulaire les enlèverait.
- Version publiée : 32 / 37. Toute modification de comportement exige un bump pour
  atteindre un appareil.

## Vérifications avant de livrer

```sh
# identité des deux copies
md5sum sources/en.chikari/src/watermark.rs sources/en.novelbuddy/src/watermark.rs

# par crate
cd sources/en.chikari && cargo +1.99.0 fmt --check
cd sources/en.chikari && cargo +1.99.0 clippy --target wasm32-unknown-unknown -- -D warnings
cd sources/en.chikari && cargo +1.99.0 test
```
