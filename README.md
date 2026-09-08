# CretSpec

CretSpec brengt productcode, een private projectspec en gedeelde ontwikkelafspraken samen in één lokale werkruimte. Het commando is **`cspec`**. De repositories blijven zelfstandig; een eindgebruiker heeft alleen de productrepository nodig.

## Installeren op Windows, Linux en macOS

Installeer Git en Node.js 22 of hoger, inclusief npm. Zorg dat Git toegang heeft tot de betrokken repositories. SSH en HTTPS met een credential manager worden ondersteund; zet geen tokens in repository-URLs.

Deze eerste versie installeer je vanuit de bronrepository:

```sh
git clone <CretSpec-repository-url> CretSpec
cd CretSpec
npm pack
npm install --global ./cretspec-0.1.0.tgz --ignore-scripts --no-audit --no-fund
cspec --version
```

Het pakket bevat een vaste kopie van de tool. Wijzigingen aan de bronmap veranderen die installatie niet. Voor een nieuwe versie pak en installeer je opnieuw. Er is nog geen npm-registerpublicatie of los installatieprogramma.

De opdrachten werken in PowerShell, Bash en Zsh. Als PowerShell een `.ps1`-launcher blokkeert, gebruik `npm.cmd` en `cspec.cmd`; wijzig daarvoor geen execution policy. De globale npm-prefix moet schrijfbaar zijn met je gebruikersaccount.

Zonder globale installatie kun je ook rechtstreeks `node bin/cspec.mjs --help` uitvoeren vanuit de CretSpec-map. Voor ontwikkeling is `npm link` beschikbaar; dat gebruikt wel de veranderende broncode.

## Eenmalig de richtlijnen instellen

Clone de centrale richtlijnenrepository apart, bijvoorbeeld als `CretAI`, en wijs die aan:

```sh
git clone <CretAI-repository-url> /pad/naar/CretAI
cspec guidelines set /pad/naar/CretAI
cspec guidelines path
```

Op Windows kan dat bijvoorbeeld `cspec guidelines set "D:/GitHub/CretAI"` zijn. Deze instelling staat in `~/.cretspec/config.json`, buiten de productrepositories. `CRETSPEC_HOME` kan een andere configuratiemap aanwijzen, bijvoorbeeld voor een geïsoleerde test.

De bewerkbare richtlijnenmap is onafhankelijk van de installatiemap van CretSpec. Na het hernoemen of verplaatsen van CretAI voer je `guidelines set` opnieuw uit.

## Een project ophalen

De spec-repository bevat eerst een geldig `project.json`, `guidelines.lock.json` en een `spec/`-map. Zie [het manifestformaat](docs/manifest.md). Een nog lege spec-repository moet je eerst met de templates uit je richtlijnenrepository invullen.

```sh
cspec project clone <Project-spec-repository-url> ./MijnProject
cspec project open ./MijnProject
```

```text
MijnProject/                    lokale werkmap, geen Git-repository
├── code/                       gewone clone van het product
├── spec/                       gewone clone van de private projectspec
├── .local/guidelines/           richtlijnen op de exact gekozen commit
├── .local/context.md            paden en actieve versie
├── .cretspec-workspace.json     lokale administratie
└── project.code-workspace      editorwerkruimte
```

De editorwerkruimte toont Code, Spec en Algemene richtlijnen. Algemene richtlijnen verwijst naar de centrale, bewerkbare clone. `.local/guidelines/` bevat de vastgezette projectversie. Een lokale conceptwijziging aan de centrale bron verandert die versie niet.

De clone volgt de standaardbranch van de code- en spec-repositories. Alleen de richtlijnen zijn gepind. De richtlijnenrepo wordt opgehaald via het manifest; de gekozen commit moet ook aanwezig zijn in je bewerkbare clone. Zo nodig voer je daar eerst `git fetch --tags` uit. Een gewijzigde tag die niet overeenkomt met de lock wordt geweigerd.

## Dagelijks gebruiken

```sh
cspec guidelines edit
cspec project open ./MijnProject --print
```

`edit` opent de centrale richtlijnenmap via je besturingssysteem. `open` opent het workspacebestand via de bestandsassociatie. VS Code ondersteunt dit formaat; andere editors kunnen de getoonde mappen afzonderlijk openen. Met `--print` wordt alleen het pad getoond. Vanuit een map binnen een geclonede werkruimte vindt `cspec project open` de omvattende werkruimte automatisch.

Een bestaande code-clone en spec-clone samenbrengen:

```sh
cspec project attach /pad/naar/code /pad/naar/spec ./MijnProject-werkruimte
```

`attach` verplaatst of wijzigt die clones niet. Vanuit een extern gekoppelde code-map moet je het workspacepad meegeven: die map ligt niet onder de omvattende werkruimte.

## Grenzen van deze versie

- De tool maakt uitsluitend nieuwe werkruimtes en overschrijft geen bestaande doelmap. Bij een fout blijft een eventuele gedeeltelijke nieuwe map staan; bronnen blijven behouden.
- Een workspace hoort buiten bestaande Git-repositories. Publieke builds blijven onafhankelijk van private specs en richtlijnen.
- Richtlijnen wijzigen en richtlijnen overnemen zijn aparte handelingen. Werk manifest en lock bij en maak voorlopig een nieuwe werkruimte om de nieuwe versie te gebruiken.
- De vastgezette richtlijnencheckout is detached, maar niet technisch schrijfbeveiligd. Een lock legt een versie vast; hij bewijst geen identiteit van een uitgever.
- Deze versie installeert geen projectdependencies, start geen applicatie en activeert geen skills of assistentconfiguratie.
- Inhoudelijke spec-workflows, Spec Kit-integratie, automatische updates en releaseautomatisering zijn nog niet geïmplementeerd.

## Ontwikkelen

Zie [CONTRIBUTING.md](CONTRIBUTING.md). De CI-matrix voert syntaxcontrole, integratietests en een installatiesmoke-test uit op Windows, Linux en macOS met Node.js 22 en 24.
