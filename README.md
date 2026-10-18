<p align="center">
  <img src="docs/screenshots/logo.jpg" width="140" alt="Meraki Logo" style="border-radius: 12px;">
</p>

<h1 align="center">Meraki Video Editor</h1>

<p align="center">
  <strong>PrecisÃ£o para sua linha do tempo. EspaÃ§o para suas ideias.</strong><br>
  <em>Montagem Â· Trama Â· Som Â· Entrega</em>
</p>

<p align="center">
  <img alt="VersÃ£o" src="https://img.shields.io/badge/versÃ£o-v1.0.0--rc1-9D4EDD?style=flat-square">
  <img alt="LicenÃ§a" src="https://img.shields.io/badge/licenÃ§a-MIT%20%2F%20Apache--2.0-00D2FF?style=flat-square">
  <img alt="Testes de referÃªncia" src="https://img.shields.io/badge/testes-74%2F74%20aprovados-2ea44f?style=flat-square">
  <img alt="Rust 2021" src="https://img.shields.io/badge/Rust-2021-dea584?style=flat-square">
  <img alt="Frontend" src="https://img.shields.io/badge/frontend-Tauri%20v2%20%2B%20SolidJS-00D2FF?style=flat-square">
  <img alt="Windows x64" src="https://img.shields.io/badge/Windows-x64-0078D4?style=flat-square">
  <img alt="Linux x64" src="https://img.shields.io/badge/Linux-x64-FCC624?style=flat-square">
</p>

---

O **Meraki** Ã© um editor de vÃ­deo desktop nÃ£o-linear (NLE) de alta performance projetado para criadores e profissionais audiovisuais que exigem precisÃ£o absoluta sem perder a fluidez criativa.

ConstruÃ­do com um motor matemÃ¡tico e transacional em **Rust**, composiÃ§Ã£o acelerada por **GPU em WGSL** e uma interface moderna e densa desenvolvida em **Tauri v2 e SolidJS**, o Meraki traz uma linguagem visual elegante em tons escuros e acentos em degradÃª ciano/violeta.

**Desenvolvido e mantido pela Equipe Meraki e Contribuidores.**

---

## ðŸŽ¨ Os Quatro EspaÃ§os Criativos

O Meraki organiza o fluxo de trabalho cinematogrÃ¡fico em quatro espaÃ§os especializados, compartilhando a mesma linha do tempo racional em tempo real:

### 1. Montagem (EdiÃ§Ã£o NÃ£o-Linear)
<p align="center">
  <img src="docs/screenshots/montagem.jpg" width="100%" alt="EspaÃ§o Montagem do Meraki" style="border-radius: 8px;">
</p>

- Linha do tempo multipistas de Ã¡udio e vÃ­deo com precisÃ£o de corte canÃ´nica (sem desvios de ponto-flutuante).
- Ferramenta **Aparar (Trim / Ripple Edit)** interativa pelas bordas do clipe.
- **Encaixe MagnÃ©tico (Snap / Tecla N)** com linhas-guia ciano para cortes, playhead e marcadores.
- **Marcadores da Linha do Tempo (Tecla M)** coloridos com anotaÃ§Ãµes de corte e batidas musicais.
- **Keyframes de AnimaÃ§Ã£o** (Linear e BÃ©zier Suave) para Escala, PosiÃ§Ã£o X/Y, Opacidade e Volume.
- **TransiÃ§Ãµes Editoriais** (DissolvÃªncia Cruzada com alÃ§as e Fades para o preto).
- Ferramenta de **Texto e TÃ­tulos Transparentes (`T`)** sobrepostos na trilha V2.

### 2. Trama (Compositor de Efeitos por NÃ³s)
<p align="center">
  <img src="docs/screenshots/trama.jpg" width="100%" alt="EspaÃ§o Trama do Meraki" style="border-radius: 8px;">
</p>

- Grafo acÃ­clico dirigido (DAG) de efeitos com tipagem estrita de portas (`Imagem`, `MÃ¡scara`, `Escalar`).
- NÃ³s de **Entrada de MÃ­dia**, **Ajuste de Cor** (brilho, contraste, saturaÃ§Ã£o) e **Desfoque** (Gaussian Blur).
- Suporte a conexÃµes curvas de BÃ©zier, Bypass rÃ¡pido de nÃ³s e desconexÃ£o em tempo real.
- ComposiÃ§Ã£o **Picture-in-Picture (PiP)** com mesclagem de alfa prÃ©-multiplicado.

### 3. Som (Mixagem e Engenharia de Ãudio)
<p align="center">
  <img src="docs/screenshots/som.jpg" width="100%" alt="EspaÃ§o Som do Meraki" style="border-radius: 8px;">
</p>

- Motor de Ã¡udio nativo de baixa latÃªncia em thread dedicada de tempo real via **CPAL** (latÃªncia medida de **~31,7 ms** no Windows WASAPI e zero underruns).
- **Mesa de Mixagem:** Faders de ganho suave em dB, botÃµes de **Mute** e **Solo** por trilha e fader de SaÃ­da Principal.
- **Medidores VU em Tempo Real:** MediÃ§Ã£o Peak e RMS com decaimento suave.
- **Equalizador ParamÃ©trico:** Curva espectral interativa acoplada a filtros biquad IIR em Rust.
- **Formas de Onda Reais:** RenderizaÃ§Ã£o de picos de amplitude reais sobre os clipes de Ã¡udio.

### 4. Entrega (ExportaÃ§Ã£o e Fila de Render)
- Fila de renderizaÃ§Ã£o determinÃ­stica offline (processa todos os frames sequencialmente sem descartes).
- PredefiniÃ§Ãµes de qualidade profissionais:
  - **YouTube â€” Qualidade Superior** (4K, 1440p, 1080p, CRF 18, Bitrate alto, faststart).
  - **TikTok / Reels / Shorts** (Vertical 9:16, 1080Ã—1920, H.264 otimizado para dispositivos mÃ³veis).
  - **Web Universal** (1080p padrÃ£o, compatibilidade mÃ¡xima).
  - **Master / Arquivamento** (ProRes 422 / H.265 de alta fidelidade).
  - **Personalizado** (Ajuste livre de codec, resoluÃ§Ã£o, taxa de quadros e bitrate).
- Progresso em tempo real (%), FPS de renderizaÃ§Ã£o, ETA e cancelamento com limpeza automÃ¡tica de arquivos temporÃ¡rios.

---

## âš¡ Destaques da Arquitetura

- **Tempo Racional Puro:** O Meraki utiliza segundos racionais exatos `Time { num: i64, den: u32 }` com intermediÃ¡rios `i128`. Zero acÃºmulo de erro de arredondamento IEEE 754.
- **Compositor de Shaders WGSL:** Shaders modernos em GPU compilados nativamente para DirectX 12 e Vulkan, com fallback automÃ¡tico em CPU.
- **Zero-Copy no IPC:** Os pixels do vÃ­deo nunca transitam serializados em JSON ou Base64. A apresentaÃ§Ã£o utiliza canal binÃ¡rio e WebGL2 dedicado.
- **Sistema de Proxies em Segundo Plano:** GeraÃ§Ã£o rÃ¡pida de proxies em 720p para ediÃ§Ã£o instantÃ¢nea de vÃ­deos pesados, mantendo os arquivos originais na exportaÃ§Ã£o final.
- **Autosave & RecuperaÃ§Ã£o:** Journaling contÃ­nuo a cada 3 minutos contra quedas inesperadas de energia ou travamentos, com suporte a **ReconexÃ£o de MÃ­dia Offline (Relink)**.

---

## ðŸ“¥ Download e InstalaÃ§Ã£o

Acesse a pÃ¡gina de lanÃ§amentos: **[v1.0.0-rc1](https://github.com/merakkicode/Meraki-Video-Editor/releases/tag/v1.0.0-rc1)**.

| Plataforma | Pacote de InstalaÃ§Ã£o |
|---|---|
| **Windows x64** | [Meraki-Setup.exe](https://github.com/merakkicode/Meraki-Video-Editor/releases/download/v1.0.0-rc1/Meraki-Setup.exe) (Instalador NSIS) Â· [Meraki.msi](https://github.com/merakkicode/Meraki-Video-Editor/releases/download/v1.0.0-rc1/Meraki.msi) |
| **Linux x64** | [Meraki.AppImage](https://github.com/merakkicode/Meraki-Video-Editor/releases/download/v1.0.0-rc1/Meraki.AppImage) Â· [meraki.deb](https://github.com/merakkicode/Meraki-Video-Editor/releases/download/v1.0.0-rc1/meraki.deb) |

### InstruÃ§Ãµes para Windows:
Execute o `Meraki-Setup.exe` ou `Meraki.msi`. O instalador cria atalhos no Menu Iniciar e na Ãrea de Trabalho com suporte a desinstalaÃ§Ã£o limpa.

### InstruÃ§Ãµes para Linux:
**AppImage (PortÃ¡til sem instalaÃ§Ã£o):**
```bash
chmod +x Meraki.AppImage
./Meraki.AppImage
```

**Debian / Ubuntu (.deb):**
```bash
sudo dpkg -i meraki.deb
sudo apt-get -f install
```

**Flatpak (Flathub):**
```bash
flatpak install flathub org.merakkicode.Meraki
flatpak run org.merakkicode.Meraki
```

---

## ðŸ› ï¸ Compilando a partir do CÃ³digo-Fonte

### PrÃ©-requisitos
- **Rust:** VersÃ£o estÃ¡vel recente (EdiÃ§Ã£o 2021).
- **Node.js:** VersÃ£o 22 LTS ou 24.
- **Windows:** Visual Studio C++ Build Tools com Windows SDK.
- **Linux:** Bibliotecas de desenvolvimento (`libwebkit2gtk-4.1-dev`, `libasound2-dev`, `libssl-dev`, etc.).

### Windows
```powershell
git clone https://github.com/merakkicode/Meraki-Video-Editor.git
cd meraki
./scripts/setup-media.ps1
cargo test --workspace --locked
cd apps/desktop
npm ci
npm test
npm run dev
```

Para gerar os instaladores de produÃ§Ã£o no Windows:
```powershell
./scripts/build-windows.ps1
```

### Linux / Ubuntu
```bash
git clone https://github.com/merakkicode/Meraki-Video-Editor.git
cd meraki
bash scripts/setup-media.sh
cargo test --workspace --locked
cd apps/desktop
npm ci
npm test
npm run dev
```

---

## ðŸ§± Estrutura dos Crates do Workspace

| Crate | Responsabilidade |
|---|---|
| `meraki-core` | AritmÃ©tica racional de tempo, taxas de quadros, timecode SMPTE e identificadores tipados. |
| `meraki-timeline` | Trilhas de Ã¡udio/vÃ­deo, clipes, mapeamento de fontes, trim, marcadores e keyframes. |
| `meraki-graph` | Grafo acÃ­clico dirigido de efeitos (DAG), verificaÃ§Ã£o de ciclos e ordenaÃ§Ã£o topolÃ³gica. |
| `meraki-engine` | MÃ¡quina de estados transacional, snapshots imutÃ¡veis e histÃ³rico de Undo monotÃ´nico. |
| `meraki-media` | Probe de mÃ­dia, catÃ¡logo de assets, decodificaÃ§Ã£o, ring buffer de frames e proxies. |
| `meraki-audio` | Motor CPAL em tempo real, buffer SPSC, mixagem, faders, medidores VU e EQ biquad. |
| `meraki-compositor` | ComposiÃ§Ã£o em GPU via WGSL e CPU, efeitos de cor/desfoque e rasterizaÃ§Ã£o de texto. |
| `meraki-project` | SerializaÃ§Ã£o `.meraki`, migraÃ§Ãµes de esquema, relink de mÃ­dia e journal de recuperaÃ§Ã£o. |
| `meraki-render` | IteraÃ§Ã£o determinÃ­stica de quadros, render queue e codificaÃ§Ã£o sequencial FFmpeg. |
| `meraki-desktop` | AplicaÃ§Ã£o desktop Tauri v2, comandos IPC, roteamento de transporte e ciclo de vida. |

---

## ðŸ¤ Comunidade e ContribuiÃ§Ã£o

ContribuiÃ§Ãµes sÃ£o muito bem-vindas! Consulte o arquivo [CONTRIBUTING.md](CONTRIBUTING.md) para diretrizes de estilo de cÃ³digo, testes e fluxo de Pull Requests. Siga tambÃ©m nosso [CÃ³digo de Conduta](CODE_OF_CONDUCT.md).

## ðŸ“„ LicenÃ§a

O cÃ³digo-fonte do Meraki Ã© licenciado sob **MIT OR Apache-2.0**, Ã  sua escolha. Consulte os arquivos [LICENSE-MIT](LICENSE-MIT) e [LICENSE-APACHE](LICENSE-APACHE).
