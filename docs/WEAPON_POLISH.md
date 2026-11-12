# Waffenqualität – Fortsetzung vom 11. November 2026

Ziel: Die vorhandenen Waffen müssen verständlich und unterscheidbar sein. Qualität hat Vorrang vor weiteren Waffen.

## Ausgangspunkt und tatsächliche Beteiligung

Claude erstellte am 11. November 74 Blindbewertungen in zehn Gruppen anhand echter Kontaktbögen. Die Expertenanalysen und Synthese wurden durch das Nutzungslimit nicht fertig. Die folgende Tabelle bewahrt die abgeschlossenen Blindbewertungen; die Zahlen sind der **Ausgangsstand**, keine neue Bewertung nach dem Polish. Codex hat diese Ergebnisse wieder aufgenommen und die unten beschriebenen Änderungen implementiert.

## Umgesetzt

- Kurze Bedienhilfe für alle 74 Waffen in `Item::hint`. Im Solo-/Online-HUD und in den Aufnahmen steht die Hilfe zur eigenen Waffe; aktive Geschosse halten sie nach dem letzten Schuss sichtbar; anschließend bleibt sie vier Sekunden als letzte Waffe lesbar. Einzelmunition wird ebenfalls angezeigt.
- 22 bisher sehr ähnliche Kreise/Punkte haben eigene Formen: Hammersektor und Hammerkopf, rotierender Pendelarm, Zangen und Kette, Richtungsdornen, Weichenpfeil, Fanggerät, Rostnebel mit gesperrtem Lauf, Prallkissen mit Auswärtspfeilen, Geisterpanzer, Fadenkreuz mit Verfolgungs-/Ausweichphase. Zeitbombe, Artillerie und Luftschlag verwenden dieselbe Zeitdarstellung.
- Wirkungsgrenzen bleiben fest; ein zusätzlicher Bogen zeigt die verbleibende Lebenszeit. Die Ringgranate füllt ausschließlich den gefährlichen Ring und lässt das sichere Zentrum frei. Zeitgesteuerte Angriffe erhalten beim tatsächlichen Verschwinden der Vorwarnung einen kurzen sichtbaren Auslöse-Effekt.
- Geschossspuren verbinden echte Positionen. Flummi, Querschläger, Eckenschuss, Zweitaktgeschoss, Distanzzünder, Kugelfresser und Enterhaken besitzen erkennbare Merkmale. Geräte und Nahkampfwaffen verwenden passende vorhandene Geräusche statt eines normalen Schusses.
- Richtungen von Hammer, Schatten, Nachbrennbild, Dorn, Pendel und Zange werden im vorhandenen Winkelfeld des Snapshots übertragen. Zuvor waren sie null; bei einem gedrehten Panzer zeigte die Darstellung in die falsche Richtung. Paketaufbau und Itemnummern bleiben gleich.
- Animierte Texte skalieren feste, vorab gefüllte Glyphengrößen. Die vorher bei Geschossweiche reproduzierten schwarzen Textbalken verschwanden damit. Die vorhandenen Menüänderungen bleiben erhalten.
- Sechs Demos wurden korrigiert: Flummi und Querschläger treffen zuerst eine Wand; Rammbock aktiviert seine Fähigkeit; Konterklinge wartet auf einen tatsächlich abwehrbaren Schuss in Nahkampfreichweite; Köder wartet auf Scharfschaltung; Kanonenschlag bekommt ein erreichbares Ziel.
- Showcase-Zeit läuft deterministisch mit der Simulation. Rundenwechsel, erneute Zeitlupe und Sonderregeln nach Wandbau werden verhindert. `PANZER_SHOWCASE_STRIDE=5` spart PNG-Zwischenbilder; die Simulation bleibt bei 60 Hz.

## Sichtprüfung wiederholen

```sh
cargo build --release -p panzer -p panzer-server --locked
python3 scripts/weapon_contact_sheet.py Ring Hammer Pendel Rust Switch Afterburn
```

Das Skript nutzt das vorhandene lokale Docker-Image `panzer-headless` mit Xvfb und Mesa, öffnet kein Desktop-Fenster und legt Kontaktbögen, fünfsekündige MP4s und ein Manifest in `target/weapon-review/` ab. Temporäre PNGs werden nach jeder Aufnahme auch bei Kodierfehlern gelöscht. `--jobs 1` bis `--jobs 4`, `--output` und `--binary` erlauben vergleichbare Aufnahmen alter/neuer Builds. Website- und Launcher-Medien werden erst nach der visuellen Abnahme ersetzt. Für die korrigierten Richtungen im Online-Spiel muss auch der Server den neuen Build verwenden.

## Geprüfter Stand

- `cargo test --release --workspace --locked --offline`: 68 bestanden, 3 absichtlich ignoriert (Bildexporte und Balance-Langlauf). Vier neue Regressionen prüfen Richtungen durch volle/Delta-Snapshots, alle 74 Szenen ohne Rundenwechsel/Sonderregeln, sichtbare Abpraller/Teilung und tatsächliche Treffer der korrigierten Nahkampf-Demos.
- Alle 74 Waffen headless aufgezeichnet: je 60 gespeicherte Frames, 74 Kontaktbögen und 74 fünfsekündige MP4s. Der Recorder bestätigt die erwartete Bildzahl und erfolgreiche Kodierung.
- Schwarze Beschriftungen an Geschossweiche vor/nach dem Glyphen-Fix angesehen. Einzelschuss-Hilfe bleibt nach dem Feuern als „Zuletzt“ lesbar; Laser und Geschossweiche wurden nach dieser Ergänzung erneut aufgenommen.
- Menü und Achtspieler-HUD zusätzlich headless geprüft. Eine parallel laufende UI-Aufgabe verändert ebenfalls `gfx.rs` und `render/hud.rs`; deren neue helle Karten und einfache Überschriften wurden nicht zurückgesetzt. Ein Commit muss diese gemeinsam bearbeiteten Dateien beachten.
- Sichtprüfungen hier unter Linux mit Mesa-Software-GL, keine neuen Windows-/macOS-GPU-Prüfungen und noch kein menschlicher Blindtest nach dem Polish.

Lokale Ergebnisse: `target/weapon-review/comparison.png`, `final/manifest.json`, `final/verification.json`, `final-hud/` und `ui-smoke/`.

## Offene Qualitätsarbeit

- Trittplatten-Betritte, Kugelfresser-Größe und der exakte Stillstandszähler des Urteils fehlen im Snapshot. Die Darstellung erfindet diese Werte nicht. Das Urteil zeigt eine Bewegungswarnung und seine Gesamtdauer; ein echter Stillstands-Fortschritt braucht übertragene Zustandsdaten.
- Eis, Bärenfalle, Enterhaken und Kettenzange teilen den Zustand `Frozen`. Die Zange besitzt jetzt eine sichtbare Kette; weitere unterschiedliche Halte-/Löseanimationen brauchen eine verlässliche Unterscheidung der Ursache.
- Phase, Plasma, Railgun und Granatwerfer brauchen Demos mit einer Wand zwischen den Panzern; Blitz und durchschlagende Waffen brauchen mehrere Ziele. Diese verbleibenden Szenen sind noch keine Belege für die besonderen Mechaniken.
- Sofortaktionen (Geschossschere, Entwaffnen, Zündpause, Konterklinge) brauchen weitere eigene Auslöse- und Trefferanimationen. Geräusche sind technisch eingebunden; eine Hörprobe und menschliche Verständlichkeitsprüfung stehen aus.
- Neue menschliche Blindtests und Balancing bleiben offen. Bot-Ergebnisse dienen hier der Regression, nicht der Balanceentscheidung.

## Zweiter Durchgang am 12. November 2026

Nur Client-Optik und Klang, Spielregeln und Netzformat unverändert.

- Mündungsfeuer als geformter Blitz (`P::Muzzle`, additive Dreiecke über `Gfx::lit_tri`), ein Blitz pro Panzer und Bild. Raketen zünden mit Rückstrahl hinter dem Panzer, die Kanone mit Rauchkranz und Druckring.
- Abpraller mit Wandnormale: Funken fliegen von der Wand weg. Sägeblatt schleift (Funkenregen, Kerbe, Klang `grind`), Kanonenkugel prallt schwer ab (Staub, Wandsplitter, Klang `thud`). Geschosse, die ohne Knall enden, verpuffen (`fizzle`).
- Neu gezeichnet: Sägeblatt, Kanonenkugel, Bumerang, Schwarzes Loch im Flug, Klebebombe (Klecks mit Countdown, über den Panzern gezeichnet), Öllache, Bärenfalle, Plasmakugel. Phasenkugel flimmert in Wänden, Fernlenkrakete zeigt die Funkverbindung, Sprungdüse die Flugbahn.
- Sofortwaffen ohne Geschoss zeigen ihre Auslösung in der echten Reichweite (`detect_action`/`draw_actions` in `render/weapons.rs`): Geschossschere, Entwaffnungsfächer, Zündpause (dazu Uhrring an angehaltenen Kugeln), Konterklinge. Erkannt wird der Einsatz an Munition bzw. Waffe im Snapshot; bei Einzelladungen enthält das Munitionsfeld die freien normalen Kugeln.
- Behoben: vertauschte Aufräum-Bedingung der Geschoss-Historie seit v2.3.1 (keine Schweife, keine Abpraller, falsche Flugrichtungen). Zielhilfe der Geschossschere halb so breit wie zuvor, also jetzt wie die echte Wirkung.
- Vorher/Nachher-Bögen: `target/weapon-review/polish-1112/`.

## Vollständiger übernommener Blindtest

Bewertung 1 = unklar, 4 = gut erkennbar. Jede hier aufgeführte Waffe hat die neue Bedienhilfe; die zusätzliche Arbeit dieses Durchgangs steht rechts.

| Waffe (Item) | Vorher | Zusätzliche Arbeit dieses Durchgangs |
| --- | ---: | --- |
| Shear | 2 | gemeinsame Bedienhilfe / Grundsysteme |
| Disarm | 2 | gemeinsame Bedienhilfe / Grundsysteme |
| Chain | 2 | eigene Darstellung |
| Hammer | 3 | eigene Darstellung |
| Parry | 1 | Demo korrigiert |
| Twist | 2 | eigene Darstellung |
| Pause | 2 | gemeinsame Bedienhilfe / Grundsysteme |
| Dist | 3 | eigene Darstellung |
| Double | 2 | eigene Darstellung |
| Corner | 2 | eigene Darstellung |
| Eater | 3 | eigene Darstellung |
| Shadow | 2 | eigene Darstellung |
| Cracker | 1 | eigene Darstellung, Demo korrigiert |
| Decoy | 1 | eigene Darstellung, Demo korrigiert |
| Ring | 3 | eigene Darstellung |
| Frag | 3 | gemeinsame Bedienhilfe / Grundsysteme |
| Rocket | 2 | gemeinsame Bedienhilfe / Grundsysteme |
| Rc | 1 | gemeinsame Bedienhilfe / Grundsysteme |
| Swarm | 3 | gemeinsame Bedienhilfe / Grundsysteme |
| Fw | 2 | gemeinsame Bedienhilfe / Grundsysteme |
| Sticky | 2 | gemeinsame Bedienhilfe / Grundsysteme |
| Gl | 4 | gemeinsame Bedienhilfe / Grundsysteme |
| Napalm | 4 | gemeinsame Bedienhilfe / Grundsysteme |
| Strike | 4 | gemeinsame Bedienhilfe / Grundsysteme |
| Arty | 3 | gemeinsame Bedienhilfe / Grundsysteme |
| Nuke | 3 | gemeinsame Bedienhilfe / Grundsysteme |
| Destruct | 3 | gemeinsame Bedienhilfe / Grundsysteme |
| Pincer | 2 | eigene Darstellung |
| Cross | 2 | eigene Darstellung |
| Verdict | 2 | eigene Darstellung |
| Laser | 4 | gemeinsame Bedienhilfe / Grundsysteme |
| Zap | 3 | gemeinsame Bedienhilfe / Grundsysteme |
| Rail | 4 | gemeinsame Bedienhilfe / Grundsysteme |
| Prism | 2 | gemeinsame Bedienhilfe / Grundsysteme |
| Orbit | 2 | gemeinsame Bedienhilfe / Grundsysteme |
| Afterburn | 2 | eigene Darstellung |
| Mg | 4 | gemeinsame Bedienhilfe / Grundsysteme |
| Shotgun | 3 | gemeinsame Bedienhilfe / Grundsysteme |
| Bouncy | 1 | Demo korrigiert |
| Split | 1 | Demo korrigiert |
| Wave | 3 | gemeinsame Bedienhilfe / Grundsysteme |
| Seek | 2 | gemeinsame Bedienhilfe / Grundsysteme |
| Phase | 2 | gemeinsame Bedienhilfe / Grundsysteme |
| Gold | 3 | gemeinsame Bedienhilfe / Grundsysteme |
| Saw | 2 | gemeinsame Bedienhilfe / Grundsysteme |
| Plasma | 2 | gemeinsame Bedienhilfe / Grundsysteme |
| Cannon | 2 | gemeinsame Bedienhilfe / Grundsysteme |
| Nova | 4 | gemeinsame Bedienhilfe / Grundsysteme |
| Swap | 3 | gemeinsame Bedienhilfe / Grundsysteme |
| Rang | 2 | gemeinsame Bedienhilfe / Grundsysteme |
| Ice | 3 | gemeinsame Bedienhilfe / Grundsysteme |
| Hole | 2 | gemeinsame Bedienhilfe / Grundsysteme |
| Jump | 2 | gemeinsame Bedienhilfe / Grundsysteme |
| Ram | 1 | Demo korrigiert |
| Hook | 3 | gemeinsame Bedienhilfe / Grundsysteme |
| Push | 3 | gemeinsame Bedienhilfe / Grundsysteme |
| Frost | 4 | gemeinsame Bedienhilfe / Grundsysteme |
| Flame | 4 | gemeinsame Bedienhilfe / Grundsysteme |
| Switch | 1 | eigene Darstellung |
| Thorn | 2 | eigene Darstellung |
| Plate | 2 | eigene Darstellung |
| Pendel | 1 | eigene Darstellung |
| Catcher | 3 | eigene Darstellung |
| Rust | 1 | eigene Darstellung |
| Cushion | 2 | eigene Darstellung |
| Mine | 4 | gemeinsame Bedienhilfe / Grundsysteme |
| Trap | 2 | gemeinsame Bedienhilfe / Grundsysteme |
| Oil | 1 | gemeinsame Bedienhilfe / Grundsysteme |
| Spider | 3 | gemeinsame Bedienhilfe / Grundsysteme |
| Timebomb | 4 | gemeinsame Bedienhilfe / Grundsysteme |
| Fence | 4 | gemeinsame Bedienhilfe / Grundsysteme |
| Wall | 2 | gemeinsame Bedienhilfe / Grundsysteme |
| Sentry | 4 | gemeinsame Bedienhilfe / Grundsysteme |
| Sprinkler | 3 | gemeinsame Bedienhilfe / Grundsysteme |
