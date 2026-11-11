# Statische pubquiz

Een kleine pubquiz zonder backend of accounts. Open `index.html` rechtstreeks in een browser.

## Gebruik

1. Open **Beheren** en geef de quiz een naam.
2. Voeg een vraag toe en kies bij **Vraagtype** voor **Alcoholtijdlijn**.
3. Pas drankjes aan, voeg eigen drankjes toe of verwijder drankjes. Je kunt de naam, het richtpercentage en de afbeelding wijzigen.
4. Controleer eventueel de richtpercentages en kies het startdrankje.
5. Open **Presenteren**. De quiz start met één correct geplaatst drankje als anker; de actieve speler sleept telkens het losse drankobject tussen de bestaande drankjes.
6. Klik op **Controleer plaatsing**. Bij een fout blijft het drankje beschikbaar voor de volgende speler; bij een goed antwoord wordt het vast onderdeel van de tijdlijn.
7. Gebruik **Antwoordenoverzicht** om de percentages en de juiste volgorde te tonen.
8. Exporteer de quiz als JSON om een back-up te maken of op een ander apparaat te gebruiken.

De laatst gebruikte quiz wordt automatisch in `localStorage` bewaard. De alcoholpercentages zijn realistische richtwaarden en kunnen per merk verschillen.

## Alcoholtijdlijn

De tijdlijn gebruikt tien aparte SVG-bestanden in `assets/drinks/`. Elk drankje is een afzonderlijk HTML-object, geen afgevlakte afbeelding. Eigen uploads worden als data-URL in de quiz opgeslagen, zodat ze ook in JSON-export/import meegaan. De speler kiest een plek tussen bestaande kaartjes; na controle wordt de resulterende volgorde vergeleken met de juiste volgorde. Er spelen drie spelers; na iedere controle gaat de beurt naar de volgende speler.

## Nieuwe vraagtypes toevoegen

Vraagtypes worden geregistreerd in `app.js` in het object `questionTypes`. Een type definieert:

- `label`: de naam in de editor;
- `create()`: de standaarddata;
- `renderEditor(question)`: de invoervelden;
- `readEditor()`: de data uit de invoervelden;
- `validate(data)`: optionele validatie;
- `renderPresentation(question)`: de weergave op het presentatiescherm.

Voeg een nieuwe entry toe naast `text` en `alcoholTimeline` en zorg dat `normalizeQuiz` de type-specifieke data behoudt zodra dat type complexere velden gebruikt.
