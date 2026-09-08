# Repositorykoppelingen en versies

De projectspec is het vertrekpunt. De code-repository bevat geen terugverwijzing naar private ontwikkelinformatie.

```text
Project-spec/
  project.json
  guidelines.lock.json
  spec/...
```

## project.json

```json
{
  "schemaVersion": 1,
  "name": "Project",
  "code": { "repository": "../Project" },
  "guidelines": { "repository": "../CretAI", "ref": "v0.1.0" },
  "profile": "dotnet"
}
```

`name` benoemt het project. `code.repository` verwijst naar de zelfstandige code-repository. `guidelines.repository` verwijst naar de centrale afspraken. `guidelines.ref` kiest een richtlijnenversie, bij voorkeur een onveranderlijke releasetag. `profile` selecteert het bestand `profiles/<profiel>.md` in die richtlijnenversie.

Relatieve verwijzingen worden opgelost ten opzichte van de **bronlocatie van de spec-repository**. `../Project` bij `git@github.com:owner/Project-spec.git` wordt `git@github.com:owner/Project`. Voor een lokale spec-bron verwijst het naar de naastliggende map. De gekozen doelmap van de workspace verandert dit niet.

Gebruik volledige HTTPS- of SSH-URLs bij verschillende eigenaars of servers. Lokale absolute paden zijn alleen geschikt voor lokale specs; een remote spec mag geen absoluut lokaal repositorypad aanwijzen. Bij `attach` bepaalt de origin van de spec de bronlocatie, met het lokale pad als fallback wanneer er geen origin is.

## guidelines.lock.json

```json
{
  "schemaVersion": 1,
  "ref": "v0.1.0",
  "commit": "VOLLEDIGE_COMMIT_VAN_DE_GEKOZEN_TAG"
}
```

Het voorbeeld bevat bewust een placeholder. Vul de volledige commit-ID in die `git rev-parse "v0.1.0^{commit}"` in de richtlijnenrepo teruggeeft. CretSpec controleert de gelijkheid van ref en commit en maakt een detached checkout op de exacte commit.

De lock legt de richtlijnen vast, niet de CretSpec-installatie of de code- en spec-HEAD. CretSpec heeft zijn eigen pakketversie. Nieuwe code en specs volgen bij clonen hun standaardbranch; reeds bestaande clones worden niet bijgewerkt.

## Wijzigingen overnemen

1. Werk richtlijnen uit in de centrale, bewerkbare clone.
2. Commit en publiceer een nieuwe richtlijnentag zonder bestaande tags te verplaatsen.
3. Kies expliciet welke projecten de nieuwe versie moeten gebruiken.
4. Werk in hun specs zowel de manifestref als de lock bij.
5. Maak een nieuwe workspace om de vastgelegde versie te gebruiken.

Een editorbestand, de persoonlijke configuratie en lokale workspaceadministratie worden niet in productcode geschreven. Spec- en codewijzigingen zijn afzonderlijke Git-commits: leg de relatie vast in de private spec wanneer gedrag is geïmplementeerd.

De machineleesbare formaten staan in [project.schema.json](../schemas/project.schema.json) en [guidelines-lock.schema.json](../schemas/guidelines-lock.schema.json). Beide vereisen schemaVersion 1; onbekende manifest- of lockvelden worden geweigerd.
