# PokéRogue Rust

**`pokerogue-rs`** é uma reimplementação experimental, offline-first, de partes do PokéRogue em Rust. O workspace separa regras e persistência do cliente Macroquad e compara recortes determinísticos com o TypeScript original.

<p align="center">
  <img src="docs/roadmap-progress.svg" alt="Progresso do checklist do roadmap e cenas visuais automatizadas" width="100%">
</p>

> **Status:** protótipo jogável em desenvolvimento. O gráfico mede itens do checklist do roadmap, não a porcentagem do jogo original portada. Os itens têm tamanhos diferentes; ainda há diferenças funcionais e visuais importantes.

## O que já existe

- Workspace Rust com `pokerogue-core`, `pokerogue-data`, `pokerogue-client` e `pokerogue-tests`.
- Batalhas offline e um mini-run de Town com combate, EXP, captura e recompensas em escopo limitado.
- Fixtures diferenciais para RNG, regras e progressão, executadas contra métodos isolados do TypeScript fixado.
- Captura visual local com Helium/Playwright e Macroquad, resolução 960×540 e relógio determinístico.
- Nove cenários visuais declarados: campo, HUD, entrada, movimentos, um quadro de Tackle e o arremesso inicial de Poké Ball.

O harness visual ainda não executa o `BattleScene` e o `PhaseManager` completos. Capturas, ataques e recompensas cobrem apenas os estágios explicitamente documentados. Baselines e imagens ficam locais; não são distribuídos.

## Compilar e jogar

Requisitos: Rust stable com suporte à edition 2024. O clone recursivo inclui o código original fixado e seus submódulos locais necessários para executar e validar o port:

```sh
git clone --recurse-submodules https://github.com/pontojasko/pokerogue-rs.git
cd pokerogue-rs
cargo build -p pokerogue-client
cargo run --offline -p pokerogue-client
```

Se o clone já foi feito sem submódulos:

```sh
git submodule update --init --recursive
```

O primeiro `cargo build` baixa as dependências Cargo se elas ainda não estiverem no cache. Depois disso, o cliente pode ser executado com `--offline` e não depende de servidores do jogo. Pressione Enter para começar; `1` e `2` escolhem movimentos; Enter reinicia após um resultado; Escape fecha.

## Compilar e testar

```sh
cargo check --workspace --offline
cargo test -p pokerogue-client --offline
python3 -m unittest discover -s scripts -p test_visual_parity.py
node tools/visual/harness.test.mjs
```

No snapshot publicado inicialmente, `cargo check --workspace --offline` e os 13 testes do cliente passam. `cargo test --workspace --offline` ainda tem uma falha conhecida: `deterministic_twenty_wave_run_clears_bosses_and_saves` perde na wave 12. Os demais testes do workspace passam; veja [PORTING_PROGRESS.md](PORTING_PROGRESS.md) para o contexto e as lacunas conhecidas.

## Testes visuais locais

Requisitos adicionais: Node, ImageMagick (`magick`), Helium ou Chromium compatível com Playwright e Xvfb para captura sem desktop. Consulte [tools/visual/README.md](tools/visual/README.md) para instalação e limites do harness.

```sh
npm ci --prefix tools/visual --ignore-scripts
python3 tools/visual/bootstrap.py --xvfb
python3 scripts/visual_parity.py --scene battle-town-capture --native-backend xvfb
```

Para preparar baselines de regressão na sua máquina, inspecione as capturas e aceite-as explicitamente:

```sh
python3 scripts/visual_suite.py --record-baselines --native-backend xvfb
python3 scripts/visual_suite.py --native-backend xvfb
```

Snapshots, imagens dos jogos, baselines, dependências baixadas e o runtime Xvfb não fazem parte deste repositório. A comparação é executada apenas com instâncias locais; ela não contata os servidores oficiais.

## Estrutura

| Caminho | Responsabilidade |
| --- | --- |
| `pokerogue-core/` | Batalha, progressão, RNG e saves locais |
| `pokerogue-data/` | Dados importados e tabelas de regras |
| `pokerogue-client/` | Renderização Macroquad e entrada |
| `pokerogue-tests/` | Testes diferenciais e de integração |
| `original/` | Submódulo TypeScript fixado usado como referência |
| `tools/visual/`, `scripts/` | Harnesses, fixtures e ferramentas de validação |
| `ROADMAP.md`, `PORTING_PROGRESS.md`, `PARITY_PLAN.md` | Escopo, progresso e limites das comparações |

## Licença e assets

O código deste port é licenciado sob **AGPL-3.0-only**; consulte [LICENSE](LICENSE). O código TypeScript original permanece no submódulo e conserva seus créditos/licença originais. Sprites, fontes e outros assets são obtidos do submódulo upstream separado, não copiados para este repositório. Os direitos e créditos ainda precisam ser avaliados para redistribuição: veja [ASSET_CREDITS.md](ASSET_CREDITS.md). Não publique screenshots, assets, baselines nem arquivos do runtime visual.

Este projeto não é afiliado aos autores de PokéRogue.
