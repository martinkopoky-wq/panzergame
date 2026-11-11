## Verbessert in 2.3.1
- Läuft flüssiger, wenn viel los ist: Der Server baut die Welt pro Takt jetzt einmal für alle und schickt nur noch Änderungen, statt sie für jeden Spieler einzeln zu rechnen (sobald niemand versteckt ist). Das Labyrinth wird nicht mehr jeden Takt grundlos neu kodiert
- Weniger Stottern im Getümmel: die Grafik macht pro Bild weniger überflüssige Arbeit (Partikel-Listen, Karten-Erneuerung nur an den echten Änderungen, Beschriftungen und Namensabkürzungen werden vorbereitet statt jede Millisekunde neu vermessen)
- Bots, Lenkraketen und Schwärme nutzen wiederverwendete Wege; nach Atombombe oder Wandbauer ist die Kollisions-Übersicht schneller neu aufgebaut
- Die Bau-Pipeline für neue Versionen ist unempfindlicher gegen Netzwerk-Störungen auf den Bau-Computern

## Wichtig
- PROTO bleibt 11: 2.2.0, 2.2.1, 2.3.0 und 2.3.1 können zusammen spielen. Die Richtungsanzeige für Hammer, Schatten und Co. (seit 2.3.0) wirkt nur, wenn Client und Server mindestens 2.3.0 sind
