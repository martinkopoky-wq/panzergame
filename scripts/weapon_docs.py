"""Texte für die Waffen-Doku (Website und Launcher).

Quelle der Namen, Klassen und Munition ist crates/core/src/items.rs. Dieses Skript
führt beides zusammen und schreibt website/data/waffen.json. Danach lädt die
Website und der Launcher dieselben Daten.

Aufruf: python3 scripts/weapon_docs.py
"""
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# id (Rust-Name) -> Kurzbeschreibung. Munition und Klasse kommen aus items.rs.
TEXT = {
    "Frag": "Feuer schießt eine große Kugel. Ein zweiter Druck zündet sie, oder sie zündet nach 5 s von selbst. Dann fliegen 26 Splitter in alle Richtungen, auch auf dich.",
    "Mg": "Feuer gedrückt halten: 25 Schuss mit Streuung.",
    "Laser": "Sofortiger Strahl, der bis zu 1400 px weit von Wänden abprallt. Mit gepunkteter Zielhilfe.",
    "Rocket": "Rakete mit Rauchspur. Sie fliegt kurz geradeaus und sucht dann den Weg durchs Labyrinth zum nächsten Gegner. Explodiert nach 10 s, wenn der Treibstoff alle ist.",
    "Shotgun": "Zwei Schuss, jeder mit sieben Schrotkugeln im Fächer, die nach gut 1 s verschwinden.",
    "Bouncy": "Pinker Ball, der mit jedem Abpraller schneller wird (bis fast 4×).",
    "Mine": "Drei Minen hinter dem Panzer. Nach 1 s sind sie scharf. Wer drüberfährt oder draufschießt, löst sie aus. Die eigenen Minen gehen auch hoch.",
    "Swap": "Trifft sie einen Panzer, tauschst du mit ihm den Platz. Ein zweiter Druck teleportiert dich zur Kugel.",
    "Rang": "Fliegt eine Kurve und kommt zurück. Fängst du ihn, kannst du ihn noch mal werfen.",
    "Ice": "Friert den getroffenen Panzer 3 s ein: kein Fahren, kein Schießen. Ein Schild fängt ihn ab.",
    "Hole": "Fliegt 1,2 s, öffnet sich dann für 4,5 s und zieht Panzer und Geschosse an. Alles in der Mitte wird verschluckt, auch durch den Schild und auch dich. Wände schützen.",
    "Strike": "Markiert einen Punkt bis 280 px voraus. Ein Jet überfliegt das Ziel, 1,5 s später schlägt dort eine Bombe ein, die alles im Umkreis trifft.",
    "Flame": "Feuer gedrückt halten: Flammenstrahl mit gut 100 px Reichweite, der an Wänden abprallt und Minen zündet. Die eigenen Flammen verbrennen dich nicht.",
    "Zap": "Trifft sofort den nächsten Panzer vor dir (bis 250 px, freie Sicht) und springt auf bis zu zwei weitere über.",
    "Rail": "Feuer gedrückt halten lädt auf (knapp 1 s). Loslassen feuert einen Strahl, der durch bis zu zwei Wände schlägt und alle Panzer in der Linie trifft.",
    "Rc": "Der Panzer bleibt stehen, Links und Rechts lenken die Rakete (6 s Treibstoff). An einer Wand oder einem Panzer geht sie hoch, ein zweiter Druck zündet sie sofort.",
    "Sentry": "Stellt einen Turm hinter den Panzer, der 9 s lang auf Gegner mit freier Sicht schießt. Seine Kugeln treffen auch dich. Ein Treffer zerstört ihn.",
    "Wall": "Drei Wandstücke quer hinter den Panzer, die nach 10 s zerfallen. Sie blocken Kugeln und Wege.",
    "Oil": "Zwei Ölflecken hinter dem Panzer. Wer drüberfährt, rutscht. Feuer oder eine Explosion zündet das Öl, es brennt 2,5 s und erwischt jeden darin.",
    "Sticky": "Zwei Schuss. Die Bombe bleibt an Wand oder Panzer kleben und geht 1,5 s später hoch.",
    "Gl": "Drei Granaten, die über Wände fliegen und 240 px voraus explodieren. Mit Zielhilfe.",
    "Swarm": "Fünf kleine Lenkraketen im Fächer.",
    "Saw": "Prallt bis zu siebenmal ab und sägt durch jeden Panzer auf dem Weg.",
    "Plasma": "Schmilzt sich durch bis zu drei Wände, die danach weg sind, und trifft jeden auf dem Weg.",
    "Fw": "Zwei Raketen, die nach 0,7 s oder an einer Wand in 16 bunte Funken zerplatzen.",
    "Push": "Zwei Stöße: schiebt Panzer im Kegel vor dir weg und lenkt Geschosse um, die dann dir gehören.",
    "Spider": "Zwei Spinnen hinter dem Panzer. Sobald ein Gegner in die Nähe kommt, krabbeln sie zu ihm und explodieren.",
    "Ram": "4 s lang schneller. Wer gerammt wird, ist erledigt.",
    "Nova": "Zwölf Kugeln auf einmal, rundherum.",
    "Seek": "Drei Schuss, die zum nächsten sichtbaren Panzer lenken, nach 1 s auch zu dir.",
    "Phase": "Zwei Schuss geradeaus durch alle Wände.",
    "Cannon": "Dicke Kugel, die durch Schilde und Panzer geht und zweimal abprallt.",
    "Arty": "Drei Einschläge hintereinander vor dir. Wände spielen keine Rolle.",
    "Trap": "Zwei Fallen hinter dem Panzer. Wer reinfährt, auch du, hängt 4 s fest.",
    "Jump": "Drei Sprünge ein Stück nach vorne, auch über Wände.",
    "Frost": "Friert alle Panzer im Umkreis von 130 px 3 s ein.",
    "Orbit": "Markiert einen Punkt 240 px voraus. 1,6 s später wandert dort ein Strahl von oben weiter und trifft alles, auch durch Schilde.",
    "Prism": "Laserstrahl, der sich an der ersten Wand in drei Strahlen aufspaltet.",
    "Split": "Kugel, die sich bei den ersten drei Abprallern jeweils verdoppelt: bis zu acht Kugeln, die auch dich treffen.",
    "Wave": "Drei Schuss, die in Wellenlinien fliegen und schwer vorherzusehen sind.",
    "Timebomb": "Zwei Bomben hinter dem Panzer. Sie ticken 3,5 s und reißen alles im Umkreis mit, auch dich.",
    "Napalm": "Zwei Schuss. Platzt an Wand, Panzer oder nach gut 1 s und hinterlässt eine Feuerfläche, die 4,5 s brennt. Wände schützen.",
    "Fence": "Zwei Schranken quer hinter dem Panzer, von Wand zu Wand. Nach 0,8 s tödlich für jeden, auch für dich. 6 s lang aktiv.",
    "Sprinkler": "Ein Gerät hinter dem Panzer, das sich dreht und gut 3 s lang Kugeln in zwei Richtungen spuckt.",
    "Hook": "Drei Schuss. In einer Wand zieht er dich hin. An einem Panzer zieht er den Gegner zu dir und friert ihn 1,4 s ein.",
    "Destruct": "Nach 1,2 s fliegst du in die Luft und reißt alles im Umkreis von 120 px mit. Nur ein Schild oder der Superstern rettet dich.",
    "Shear": "Schneidet im Kegel vor dem Panzer (150 px, 60°) bis zu sechs gegnerische Kugeln ab. Deine eigenen bleiben.",
    "Ring": "Wirft den Ring 200 px voraus. Nach 1,1 s explodiert er und trifft nur Panzer zwischen 35 und 85 px vom Zentrum. Im Zentrum bist du sicher.",
    "Dist": "Eine Kugel mit 400 px Reichweite. Erst nach 140 px Flugstrecke ist sie scharf. Auf kurze Distanz wirkungslos.",
    "Eater": "Schluckt bis zu drei normale gegnerische Kugeln. Dabei wird er größer und etwas langsamer.",
    "Disarm": "Kegel vor dem Panzer, 110 px weit. Gegner darin verlieren ihre Waffe. Sie liegt als Kiste am Boden und kann 0,7 s lang nicht aufgenommen werden.",
    "Shield": "Fängt 8 s lang einen Treffer ab und blinkt kurz vor Ablauf.",
    "Turbo": "6 s lang schneller fahren und lenken.",
    "Mini": "10 s lang nur halb so groß. Schwerer zu treffen, etwas schneller.",
    "Ghost": "5 s lang durch Wände fahren. Solange du in einer Wand steckst, kannst du nicht schießen.",
    "Invis": "8 s lang für die anderen unsichtbar. Lenkraketen finden dich nicht.",
    "Confuse": "Alle anderen haben 6 s lang vertauschte Steuerung: vorne und hinten, links und rechts.",
    "Mirror": "7 s lang prallen Kugeln an dir ab und fliegen zum Schützen zurück.",
    "Star": "6 s lang unverwundbar, außer gegen Schwarzes Loch und Atombombe. Wen du anfährst, ist erledigt.",
    "Giant": "9 s lang anderthalbmal so groß und etwas langsamer. Kleinere Panzer werden zertrampelt.",
    "Rapid": "8 s lang doppelt so viele Kugeln und doppelt so schnell nachschießen.",
    "Extra": "Rettet einmal vor einem Treffer, sogar vor der Atombombe, und hält die ganze Runde.",
    "Slow": "Alle anderen fahren 5 s lang nur halb so schnell.",
    "Twin": "10 s lang zwei Kugeln pro Schuss.",
    "Tp": "Bringt dich sofort an die Stelle, die am weitesten von allen anderen weg ist.",
    "Disco": "Alle Panzer tauschen reihum die Plätze.",
    "Radar": "12 s lang siehst du getarnte Panzer, Minen, Fallen und Spinnen.",
    "Nuke": "Nur aus der Wundertüte. Die Bombe rollt aus und geht nach 4 s mit 230 px Radius hoch. Sie geht durch Wände und Schilde und sprengt die Wände in der Nähe weg. Die Außenwände bleiben.",
    "Gold": "Nur aus der Wundertüte. Eine schnelle Kugel, die durch Schilde und durch jeden Panzer auf dem Weg geht.",
    "Potato": "Nur aus der Wundertüte. Die Kartoffel explodiert nach 10 s. Fährst du einen anderen Panzer an, hat er sie, mit mindestens 3 s Restzeit.",
    "Meteor": "Nur aus der Wundertüte. Neun Einschläge irgendwo im Labyrinth, auch bei dir.",
    "Double": "Ein Treffer markiert den Gegner nur, er bleibt 2 s lang markiert. Trifft derselbe Schütze in dieser Zeit noch einmal, ist er erledigt. Der Erste schadet nicht.",
    "Corner": "Der zweite Feuerdruck biegt das Geschoss einmal um 90° nach rechts ab. So kommt man um Ecken herum.",
    "Chain": "Packt den Gegner im Frontwinkel von 30° innerhalb von 38 px. Beide stehen 0,9 s still, danach trifft die Zange das Opfer, solange der Angreifer noch lebt.",
    "Hammer": "Holt 0,45 s aus und schlägt dann durch einen Sektor von 120° bis 48 px vor dem Panzer. Hinter dem Panzer bist du sicher.",
    "Pause": "Friert alle normalen Kugeln im Umkreis von 110 px für 0,9 s ein. Sie stehen still und sind dabei harmlos. Danach fliegen sie weiter.",
    "Decoy": "Wirft eine Ladung 160 px voraus, die nach 0,8 s scharf wird. Fliegt eine gegnerische Kugel nah genug vorbei, zündet sie nach 0,35 s und reißt alles im Umkreis von 55 px mit.",
    "Shadow": "Hinterlässt eine Silhouette auf der Stelle, an der du stehst. Nach 0,9 s feuert sie drei Kugeln in deine alte Blickrichtung, auch wenn du dich schon bewegt hast.",
    "Twist": "Nach 0,5 s Vorwarnung drehen sich alle Panzer im Umkreis von 100 px einmal um 90°. Wer gerade zielt, zielt danach daneben.",
    "Switch": "Stellt eine Weiche 120 px voraus auf. Gegnerische Kugeln, die in ihren Bereich fliegen, werden um 90° nach links abgelenkt. Höchstens fünf Kugeln, 2 s lang.",
    "Afterburn": "Zeichnet eine Linie von 320 px in deine Blickrichtung. Nach 0,9 s trifft sie jeden Panzer, der dann noch auf der Linie steht. Wer ausweicht, ist sicher.",
    "Thorn": "Legt eine Leiste hinter den Panzer, die nach 0,7 s scharf wird. Nur Gegner, die von der Pfeilseite darüberfahren, lösen sie aus. Der Schütze kann drüberfahren, ohne dass etwas passiert.",
    "Plate": "Eine Platte hinter dem Panzer zählt jedes Betreten. Beim dritten Betreten warnt sie 0,7 s lang und explodiert dann im Umkreis von 65 px. Ohne Betreten bleibt sie acht Sekunden liegen.",
    "Pincer": "Markiert einen Punkt 240 px voraus. Zwei Zangen starten links und rechts davon und laufen zur Mitte. Wer dort steht, wird getroffen.",
    "Cross": "Markiert einen sichtbaren Gegner im Umkreis von 260 px mit einem Fadenkreuz. Es folgt ihm 0,8 s, friert dann ein und trifft nach weiteren 0,65 s alles im Kreis.",
    "Pendel": "Ein Arm von 65 px dreht sich um einen Sockel vor dem Panzer. Nach 0,8 s Vorwarnung trifft er jeden, der ihm in den Weg kommt, außer dem Schützen. Der Arm bleibt etwa 5 s.",
    "Verdict": "Markiert den nächsten sichtbaren Gegner im Umkreis von 220 px. Bleibt er 1,1 s lang stehen, trifft das Urteil. Bewegt er sich, beginnt die Zeit von vorn. Die Markierung hält 2,5 s.",
    "Catcher": "Stellt einen Fänger hinter den Panzer. Er fängt bis zu drei gegnerische Kugeln im Umkreis von 28 px auf und schickt sie zurück zum Schützen. Der Fänger hält 4 s.",
    "Parry": "Wehrt die erste gegnerische Kugel im Frontwinkel von 70° ab, die nah genug ist. Nur dann stößt die Klinge 45 px weit nach vorn. Ohne abgewehrte Kugel passiert nichts.",
    "Rust": "Wirft eine Wolke 180 px voraus, die 3 s bleibt. Wer in der Wolke steht, kann nicht schießen, und danach noch 1,2 s lang nicht.",
    "Cushion": "Wirft ein Kissen 180 px voraus. Nach 0,6 s ist es scharf und hält 5 s. Wer neu hineinfährt, wird um 90 px weggeschoben.",
    "Cracker": "Legt einen Kanonenschlag hinter den Panzer. Nach 1 s zündet er und wirft drei kleine Splitter in zufällige Richtungen.",
}

# Anzeige-Gruppe: Waffen aus Kisten, Effekte aus Kisten, nur Wundertüte
GROUP = {"Weapon": "Waffe", "MysteryWeapon": "Wundertüte", "Buff": "Effekt", "MysteryBuff": "Wundertüte"}


def items():
    src = (ROOT / "crates/core/src/items.rs").read_text(encoding="utf-8")
    found = re.findall(r'^\s+(\w+) = "([^"]+)", (\w+), (\d+);', src, re.M)
    return [
        {"id": i, "name": n, "class": c, "group": GROUP[c], "ammo": int(a)}
        for i, n, c, a in found
        if c in GROUP
    ]


def main():
    data = items()
    missing = [d["id"] for d in data if d["id"] not in TEXT]
    if missing:
        raise SystemExit(f"Beschreibung fehlt für: {missing}")
    for d in data:
        d["text"] = TEXT[d["id"]]
        d["video"] = f"media/waffen/{d['id'].lower()}.mp4"
        d["poster"] = f"media/waffen/{d['id'].lower()}.jpg"
    out = ROOT / "website/data/waffen.json"
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(data, ensure_ascii=False, indent=1), encoding="utf-8")
    print(f"{len(data)} Einträge nach {out}")


if __name__ == "__main__":
    main()
