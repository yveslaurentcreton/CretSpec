# Werken aan CretSpec

Gebruik Git en Node.js 22 of hoger. Er zijn geen aanvullende runtime-libraries.

```sh
npm run check
npm test
```

De tests gebruiken tijdelijke lokale Git-repositories. De CI-matrix controleert Windows, Linux en macOS met Node.js 22 en 24. Tests mogen de persoonlijke configuratie niet gebruiken: geef een tijdelijke CRETSPEC_HOME mee.

Gebruik voor ontwikkeling `node bin/cspec.mjs` of `npm link`. Een geïnstalleerd pakket heeft geen Git-checkout nodig. Haal bewerkbare richtlijnen uitsluitend uit de ingestelde bron, nooit uit de installatiemap van de tool.

Houd Git-opdrachten als argumentlijsten zonder shellinterpolatie. Overschrijf geen bestaande doelmappen en voer geen scripts uit een projectspec uit. Een fout mag gedeeltelijke nieuwe werkbestanden achterlaten, maar geen bestaande bronbestanden wijzigen.

Wijzigingen aan manifestvelden vereisen een overeenkomstige wijziging in validatie, schema's en documentatie. Voeg bij gedragswijzigingen een test met een bruikbaar gebruikersscenario toe.
