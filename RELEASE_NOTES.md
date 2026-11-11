## Neu in 2.3.0
- Oberfläche neu aufgebaut: zuerst die Spielwahl (Solo, Lokal, Hosten, Beitreten), danach nur noch die passenden Einstellungen. Das Labyrinth läuft als lebende Vorschau, der eigene Panzer hat im Spiel ein großes Waffenfeld. Der Launcher trennt Spielen, Waffen und Neuigkeiten
- Alle 74 Waffen zeigen eine kurze Bedienhilfe, im HUD und in den Listen
- 22 Spezialwaffen haben eine eigene Darstellung statt ähnlicher Punkte: eigene Formen, Anzeige der Wirkungsgrenze und der restlichen Lebenszeit, passende Geräusche

## Behoben
- Hammer, Schatten, Nachbrennbild, Richtungsdorn, Kreuzpendel und Zangenmarkierung zeigten im Online-Spiel in die falsche Richtung, weil ihre Richtung bisher als 0 übertragen wurde. Sie steckt jetzt im vorhandenen Winkelfeld des Snapshots, sichtbar, sobald Client und Server auf 2.3.0 sind
- Schwarze Balken in animierten Texten (Glyphen werden in festen Größen gecacht und dann skaliert)
- Sechs irreführende Waffenvorführungen korrigiert (unter anderem Flummi, Querschläger, Rammbock, Konterklinge, Köder, Kanonenschlag)

## Wichtig
- PROTO bleibt 11, 2.2.0, 2.2.1 und 2.3.0 können zusammen spielen
