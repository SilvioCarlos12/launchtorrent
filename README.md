# LaunchTorrent

Cliente BitTorrent desktop em Rust: UI nativa (`eframe`/`egui`) + motor `librqbit` + `tokio`.

## Recursos (MVP)

- Abrir arquivos `.torrent` (vários de uma vez) e links `magnet:`
- Arrastar e soltar `.torrent` na janela
- Barra lateral com filtros (Todos, Baixando, Pausados, Concluídos, Com erro) e contadores
- Lista com progresso, tempo restante, velocidades ↓/↑ e peers
- Pausar / retomar / abrir pasta / remover (com opção de apagar os arquivos)
- Barra de status com estado do motor e velocidades totais
- Pasta de download configurável (padrão: `Downloads/LaunchTorrent`)
- Tema slate/teal com barra lateral escura (fontes Segoe UI/Consolas no Windows)

## Requisitos

- Windows 10/11
- [Rust](https://rustup.rs/) (edição 2021+)

## Build e execução

```powershell
cd D:\lauchTorrent
cargo build
cargo run
```

Release:

```powershell
cargo build --release
.\target\release\launchtorrent.exe
```

## Uso

1. Cole um link `magnet:` e clique **Adicionar**, ou use **Abrir .torrent**.
2. Acompanhe progresso e velocidades na lista.
3. Use **Pausar** / **Retomar** / **Remover** por torrent.
4. Em **Alterar…**, escolha a pasta de destino (salva em `%APPDATA%\LaunchTorrent\config.toml`).

## Arquitetura

| Arquivo | Papel |
|---------|--------|
| `src/main.rs` | Boot da janela e logging |
| `src/app.rs` | UI egui e ponte de comandos |
| `src/engine.rs` | Sessão `librqbit` em runtime tokio |
| `src/theme.rs` | Tokens de cor e widgets |
| `src/config.rs` | Persistência da pasta de download |

A thread da UI envia comandos por canal; o motor publica snapshots a cada ~500 ms.

## Segurança das dependências

Auditoria em **2026-11-24** com RustSec (`cargo audit`) + notas de release do rqbit **9.1.0** (`librqbit = "9.1"`).

### librqbit 9.1.0 — preferido

- Inclui correções a crashes remotos: limite de recursão no bencode, bounds check em `ut_metadata`, accept do listener sem panic.
- UPnP já puxa `quick-xml 0.41.0` (corrigido).

### Achados do `cargo audit`

| ID | Crate | Impacto no LaunchTorrent (Windows) | Mitigação |
|----|--------|-------------------------------------|-----------|
| [RUSTSEC-2026-0293](https://rustsec.org/advisories/RUSTSEC-2026-0293) | `ringbuf` 0.4.8 via `librqbit-utp` | Baixo | Double-free só se `Drop` panicar; o uTP usa `SharedRb<Heap<u8>>` (`u8` sem `Drop`). Sem versão segura no uTP atual. |
| [RUSTSEC-2026-0194](https://rustsec.org/advisories/RUSTSEC-2026-0194) / [0195](https://rustsec.org/advisories/RUSTSEC-2026-0195) | `quick-xml` 0.30.0 | **Não aplicável no Windows** | Só entra via `eframe` → AccessKit/AT-SPI (`zbus_xml`) em alvos Unix. No Windows o lockfile usa `quick-xml 0.41` via UPnP. |
| Vários `unmaintained` | `chrono`, `paste`, `ttf-parser`, `zbus_*`… | Informativo | Transitivos de egui/plataforma; sem fix drop-in. |

**Veredito:** sem vulnerabilidade crítica explorável no grafo Windows do MVP. Não há CVE crítico no próprio `librqbit` 9.1.0 que bloqueie o uso. Reavalie após upgrades:

```powershell
cargo install cargo-audit
cargo audit
```

## Licença

MIT (código do app). `librqbit` é Apache-2.0.
