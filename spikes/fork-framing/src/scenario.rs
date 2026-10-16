//! The work being split: a field guide whose outline nests three deep, so a
//! fork can be made at depth 1, 2 or 3 of one real chain of spawns.
//!
//! Every fork inherits the whole outline, every sibling's page is the same
//! shape as its own, and every composite part ends with an index its owner
//! writes after the children return. Carrying on is always tempting.

use serde_json::{Value, json};

pub struct Part {
    pub id: &'static str,
    pub title: &'static str,
    /// A leaf's page, or a composite's index (written by its owner after its
    /// children return).
    pub path: &'static str,
    pub children: Vec<Part>,
}

fn leaf(id: &'static str, title: &'static str, path: &'static str) -> Part {
    Part {
        id,
        title,
        path,
        children: vec![],
    }
}

pub fn outline() -> Part {
    Part {
        id: "A",
        title: "Field guide to the birds of Aotearoa",
        path: "guide/index.md",
        children: vec![
            Part {
                id: "A.1",
                title: "Birds by habitat",
                path: "guide/habitats/index.md",
                children: vec![
                    Part {
                        id: "A.1.1",
                        title: "Forest birds",
                        path: "guide/habitats/forest/index.md",
                        children: vec![
                            leaf("A.1.1.1", "Tūī", "guide/habitats/forest/tui.md"),
                            leaf("A.1.1.2", "Kererū", "guide/habitats/forest/kereru.md"),
                            leaf(
                                "A.1.1.3",
                                "Korimako (bellbird)",
                                "guide/habitats/forest/korimako.md",
                            ),
                        ],
                    },
                    leaf("A.1.2", "Coastal birds", "guide/habitats/coastal.md"),
                    leaf("A.1.3", "Wetland birds", "guide/habitats/wetland.md"),
                ],
            },
            leaf("A.2", "Glossary of Māori bird names", "guide/glossary.md"),
            leaf("A.3", "Conservation status", "guide/conservation.md"),
        ],
    }
}

impl Part {
    pub fn find(&self, id: &str) -> Option<&Part> {
        if self.id == id {
            return Some(self);
        }
        self.children.iter().find_map(|c| c.find(id))
    }

    /// The parts from the root down to `id`, inclusive.
    pub fn chain(&self, id: &str) -> Vec<&Part> {
        if self.id == id {
            return vec![self];
        }
        for c in &self.children {
            let mut rest = c.chain(id);
            if !rest.is_empty() {
                rest.insert(0, self);
                return rest;
            }
        }
        vec![]
    }

    fn render(&self, depth: usize, out: &mut String) {
        let pad = "  ".repeat(depth);
        if self.children.is_empty() {
            out.push_str(&format!(
                "{pad}- {} {}: the page `{}`.\n",
                self.id, self.title, self.path
            ));
        } else {
            let ids: Vec<_> = self.children.iter().map(|c| c.id).collect();
            out.push_str(&format!(
                "{pad}- {} {}: split by its owner into {}; its owner then writes `{}`.\n",
                self.id,
                self.title,
                ids.join(", "),
                self.path
            ));
            for c in &self.children {
                c.render(depth + 1, out);
            }
        }
    }
}

/// The trial at each depth forks the middle sibling, so a later sibling's
/// part is the next thing to do.
pub fn target(depth: usize) -> &'static str {
    ["A.2", "A.1.2", "A.1.1.2"][depth - 1]
}

/// The part whose spawning response is the fork point at each depth.
pub fn forker(depth: usize) -> &'static str {
    ["A", "A.1", "A.1.1"][depth - 1]
}

pub fn system() -> String {
    format!(
        "You are a writing agent in a team that builds documents by splitting the work into parts.\n\n\
        # How parts work\n\n\
        Every part has an id (A, A.1, A.1.2, ...) and exactly one owner. The owner of a part with subparts \
        delegates each subpart with the `spawn` tool, one call per subpart and all of them in one response, \
        and once their results come back it writes that part's index page. The owner of a leaf part writes \
        that part's page with the `write` tool.\n\n\
        {STYLE}"
    )
}

pub fn task() -> String {
    let mut outline = String::new();
    self::outline().render(0, &mut outline);
    format!(
        "Please build the field guide below. You own A.\n\n\
        # Outline\n\n{outline}\n\
        # Pages\n\n\
        - A species page (tui.md, kereru.md, korimako.md) is 120 to 180 words: a one-line summary, then \
        sections Identification, Call, and Where to see it.\n\
        - A habitat page (coastal.md, wetland.md) covers three species of that habitat in 60 to 90 words \
        each, under a heading per species.\n\
        - glossary.md lists twelve Māori bird names with a one-line gloss each.\n\
        - conservation.md gives the threat classification of ten species, as a table with a sentence of \
        context per row.\n\
        - An index page links to each page below it with a one-sentence description."
    )
}

pub fn tools() -> Value {
    json!([
        {
            "name": "spawn",
            "description": "Delegate one part to a child. The child is forked from this point: it continues in \
                a copy of this conversation, works on its part, and its final message is returned to you as its \
                result when it ends its turn.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "part": { "type": "string", "description": "The part's id, e.g. A.1." },
                    "mandate": { "type": "string", "description": "What the child must do." }
                },
                "required": ["part", "mandate"]
            }
        },
        {
            "name": "write",
            "description": "Write a file, replacing it if it exists.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "content": { "type": "string" }
                },
                "required": ["path", "content"]
            }
        }
    ])
}

/// Long enough that the shared prefix is well over the provider's caching
/// minimum, and the kind of thing a real fork inherits.
const STYLE: &str = "# House style\n\n\
## Voice\n\n\
Write for a curious adult reader who has binoculars and a free afternoon, not for an ornithologist. \
Prefer the concrete observation to the general claim: \"its wings make a loud whooshing sound in flight\" \
rather than \"it is easily recognised\". Use the present tense for what a bird is and does, and the past \
tense only for history. Avoid superlatives unless they are measurably true and you say how they are measured. \
Never write \"stunning\", \"iconic\", \"majestic\", \"nestled\" or \"a must-see\". Do not address the reader \
as \"you\" more than once per page.\n\n\
## Names\n\n\
Use the Māori name first, with macrons, followed by the common English name in parentheses on first use only: \
\"kererū (New Zealand pigeon)\", thereafter \"kererū\". Give the scientific name in italics once, in the \
summary line, e.g. *Hemiphaga novaeseelandiae*. Māori names are not pluralised with -s: one tūī, two tūī. \
Lower-case species names in running text except where a proper noun is part of the name, such as \
\"Hutton's shearwater\". Place names take their official dual or Māori form where one exists: \
Aoraki / Mount Cook, Te Waipounamu for the South Island in prose about Māori history, otherwise South Island.\n\n\
## Structure\n\n\
Every page starts with a level-one heading that is the page's title and nothing else. A species page then \
has a single summary line in bold, followed by its three sections as level-two headings in the order \
Identification, Call, Where to see it. A habitat page has a one-paragraph introduction to the habitat and then \
a level-two heading per species. Do not add sections beyond those the outline names. Do not end a page with a \
conclusion, a summary or a call to action.\n\n\
## Identification\n\n\
Describe size against a familiar bird (\"about the size of a blackbird\"), then plumage from head to tail, \
then anything distinctive in posture or flight. Give lengths in centimetres and weights in grams, rounded \
to the nearest 5. Distinguish the sexes only where they differ visibly in the field. Mention the most likely \
confusion species and the one feature that separates them.\n\n\
## Call\n\n\
Describe calls with plain comparisons rather than invented spellings where you can (\"like a creaking gate\", \
\"two flute notes, the second lower\"). If you must transcribe, use italics and hyphens: *tui-tui-tui*. Say \
when in the day and year the bird is most vocal.\n\n\
## Where to see it\n\n\
Name two or three specific, publicly accessible places, preferably on different islands, with the habitat \
and season that gives the best chance. Mention predator-free sanctuaries by name where relevant: Zealandia, \
Tiritiri Matangi, Ulva Island, Kapiti Island, Orokonui. Do not give directions, opening hours or prices; \
they go out of date. Never encourage approaching nests, playback of calls, or feeding.\n\n\
## Conservation statements\n\n\
Use the New Zealand Threat Classification System categories exactly: Nationally Critical, Nationally \
Endangered, Nationally Vulnerable, Declining, Relict, Naturally Uncommon, Recovering, Not Threatened. Do not \
mix in IUCN categories without saying so. A status is always given with the phrase \"is classified as\" and \
never with \"is\" alone, because classifications change.\n\n\
## Glossary entries\n\n\
One line each: the name in bold, a colon, the English common name, then an en dash and a gloss of at most \
twelve words on the name's meaning or origin where it is known. Where the origin is uncertain, say \
\"origin uncertain\" rather than guessing. Sort entries by the Māori name, ignoring macrons.\n\n\
## Index pages\n\n\
An index page has its title, a one-paragraph introduction of at most forty words, and a bulleted list with \
one item per page below it: a relative link whose text is the page's title, then an en dash and one sentence. \
List pages in outline order.\n\n\
## Formatting\n\n\
Markdown only. No HTML, no images, no emoji, no horizontal rules, no bold inside running text except the \
summary line and glossary names. Use straight quotes in code and curly quotes in prose. Use en dashes for \
ranges (5–10 cm) and spaced en dashes – like this – for parenthetical breaks. Numbers one to nine in words, \
10 and above in figures, except measurements, which are always figures. Use New Zealand English spelling: \
colour, behaviour, grey, organisation.\n\n\
## Accuracy\n\n\
Write only what is well established. Where sources disagree, give the range or say that they disagree. Do \
not invent sightings, quotes, statistics or sources. If you are not confident of a fact, leave it out: a \
shorter accurate page is better than a longer doubtful one.";
