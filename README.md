<p align="center">
  <img src="docs/screenshots/logo.jpg" width="140" alt="Meraki Logo" style="border-radius: 12px;">
</p>

<h1 align="center">Meraki Video Editor</h1>

<p align="center">
  <strong>Precisão para sua linha do tempo. Espaço para suas ideias.</strong><br>
  <em>Montagem · Trama · Som · Entrega</em>
</p>

<p align="center">
  <img alt="Versão" src="https://img.shields.io/badge/versão-v1.0.0--rc1-9D4EDD?style=flat-square">
  <img alt="Licença" src="https://img.shields.io/badge/licença-MIT%20%2F%20Apache--2.0-00D2FF?style=flat-square">
  <img alt="Testes de referência" src="https://img.shields.io/badge/testes-74%2F74%20aprovados-2ea44f?style=flat-square">
  <img alt="Rust 2021" src="https://img.shields.io/badge/Rust-2021-dea584?style=flat-square">
  <img alt="Frontend" src="https://img.shields.io/badge/frontend-Tauri%20v2%20%2B%20SolidJS-00D2FF?style=flat-square">
  <img alt="Windows x64" src="https://img.shields.io/badge/Windows-x64-0078D4?style=flat-square">
  <img alt="Linux x64" src="https://img.shields.io/badge/Linux-x64-FCC624?style=flat-square">
</p>

---

O **Meraki** é um editor de vídeo desktop não-linear (NLE) de alta performance projetado para criadores e profissionais audiovisuais que exigem precisão absoluta sem perder a fluidez criativa.

Construído com um motor matemático e transacional em **Rust**, composição acelerada por **GPU em WGSL** e uma interface moderna e densa desenvolvida em **Tauri v2 e SolidJS**, o Meraki traz uma linguagem visual elegante em tons escuros e acentos em degradê ciano/violeta.

**Desenvolvido e mantido pela Equipe Meraki e Contribuidores.**

---

## 🎨 Os Quatro Espaços Criativos

O Meraki organiza o fluxo de trabalho cinematográfico em quatro espaços especializados, compartilhando a mesma linha do tempo racional em tempo real:

### 1. Montagem (Edição Não-Linear)
<p align="center">
  <img src="docs/screenshots/montagem.jpg" width="100%" alt="Espaço Montagem do Meraki" style="border-radius: 8px;">
</p>

- Linha do tempo multipistas de áudio e vídeo com precisão de corte canônica (sem desvios de ponto-flutuante).
- Ferramenta **Aparar (Trim / Ripple Edit)** interativa pelas bordas do clipe.
- **Encaixe Magnético (Snap / Tecla N)** com linhas-guia ciano para cortes, playhead e marcadores.
- **Marcadores da Linha do Tempo (Tecla M)** coloridos com anotações de corte e batidas musicais.
- **Keyframes de Animação** (Linear e Bézier Suave) para Escala, Posição X/Y, Opacidade e Volume.
- **Transições Editoriais** (Dissolvência Cruzada com alças e Fades para o preto).
- Ferramenta de **Texto e Títulos Transparentes (`T`)** sobrepostos na trilha V2.

### 2. Trama (Compositor de Efeitos por Nós)
<p align="center">
  <img src="docs/screenshots/trama.jpg" width="100%" alt="Espaço Trama do Meraki" style="border-radius: 8px;">
</p>

- Grafo acíclico dirigido (DAG) de efeitos com tipagem estrita de portas (`Imagem`, `Máscara`, `Escalar`).
- Nós de **Entrada de Mídia**, **Ajuste de Cor** (brilho, contraste, saturação) e **Desfoque** (Gaussian Blur).
- Suporte a conexões curvas de Bézier, Bypass rápido de nós e desconexão em tempo real.
- Composição **Picture-in-Picture (PiP)** com mesclagem de alfa pré-multiplicado.

### 3. Som (Mixagem e Engenharia de Áudio)
<p align="center">
  <img src="docs/screenshots/som.jpg" width="100%" alt="Espaço Som do Meraki" style="border-radius: 8px;">
</p>

- Motor de áudio nativo de baixa latência em thread dedicada de tempo real via **CPAL** (latência medida de **~31,7 ms** no Windows WASAPI e zero underruns).
- **Mesa de Mixagem:** Faders de ganho suave em dB, botões de **Mute** e **Solo** por trilha e fader de Saída Principal.
- **Medidores VU em Tempo Real:** Medição Peak e RMS com decaimento suave.
- **Equalizador Paramétrico:** Curva espectral interativa acoplada a filtros biquad IIR em Rust.
- **Formas de Onda Reais:** Renderização de picos de amplitude reais sobre os clipes de áudio.

### 4. Entrega (Exportação e Fila de Render)
- Fila de renderização determinística offline (processa todos os frames sequencialmente sem descartes).
- Predefinições de qualidade profissionais:
  - **YouTube — Qualidade Superior** (4K, 1440p, 1080p, CRF 18, Bitrate alto, faststart).
  - **TikTok / Reels / Shorts** (Vertical 9:16, 1080×1920, H.264 otimizado para dispositivos móveis).
  - **Web Universal** (1080p padrão, compatibilidade máxima).
  - **Master / Arquivamento** (ProRes 422 / H.265 de alta fidelidade).
  - **Personalizado** (Ajuste livre de codec, resolução, taxa de quadros e bitrate).
- Progresso em tempo real (%), FPS de renderização, ETA e cancelamento com limpeza automática de arquivos temporários.

---

## ⚡ Destaques da Arquitetura

- **Tempo Racional Puro:** O Meraki utiliza segundos racionais exatos `Time { num: i64, den: u32 }` com intermediários `i128`. Zero acúmulo de erro de arredondamento IEEE 754.
- **Compositor de Shaders WGSL:** Shaders modernos em GPU compilados nativamente para DirectX 12 e Vulkan, com fallback automático em CPU.
- **Zero-Copy no IPC:** Os pixels do vídeo nunca transitam serializados em JSON ou Base64. A apresentação utiliza canal binário e WebGL2 dedicado.
- **Sistema de Proxies em Segundo Plano:** Geração rápida de proxies em 720p para edição instantânea de vídeos pesados, mantendo os arquivos originais na exportação final.
- **Autosave & Recuperação:** Journaling contínuo a cada 3 minutos contra quedas inesperadas de energia ou travamentos, com suporte a **Reconexão de Mídia Offline (Relink)**.

---

## 📥 Download e Instalação

Acesse a página de lançamentos: **[v1.0.0-rc1](https://github.com/merakkicode/Meraki-Video-Editor/releases/tag/v1.0.0-rc1)**.

| Plataforma | Pacote de Instalação |
|---|---|
| **Windows x64** | [Meraki-Setup.exe](https://github.com/merakkicode/Meraki-Video-Editor/releases/download/v1.0.0-rc1/Meraki-Setup.exe) (Instalador NSIS) · [Meraki.msi](https://github.com/merakkicode/Meraki-Video-Editor/releases/download/v1.0.0-rc1/Meraki.msi) |
| **Linux x64** | [Meraki.AppImage](https://github.com/merakkicode/Meraki-Video-Editor/releases/download/v1.0.0-rc1/Meraki.AppImage) · [meraki.deb](https://github.com/merakkicode/Meraki-Video-Editor/releases/download/v1.0.0-rc1/meraki.deb) |

### Instruções para Windows:
Execute o `Meraki-Setup.exe` ou `Meraki.msi`. O instalador cria atalhos no Menu Iniciar e na Área de Trabalho com suporte a desinstalação limpa.

### Instruções para Linux:
**AppImage (Portátil sem instalação):**
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

## 🛠️ Compilando a partir do Código-Fonte

### Pré-requisitos
- **Rust:** Versão estável recente (Edição 2021).
- **Node.js:** Versão 22 LTS ou 24.
- **Windows:** Visual Studio C++ Build Tools com Windows SDK.
- **Linux:** Bibliotecas de desenvolvimento (`libwebkit2gtk-4.1-dev`, `libasound2-dev`, `libssl-dev`, etc.).

### Windows
```powershell
git clone https://github.com/merakkicode/Meraki-Video-Editor.git
cd Meraki-Video-Editor
./scripts/setup-media.ps1
cargo test --workspace --locked
cd apps/desktop
npm ci
npm test
npm run dev
```

Para gerar os instaladores de produção no Windows:
```powershell
./scripts/build-windows.ps1
```

### Linux / Ubuntu
```bash
git clone https://github.com/merakkicode/Meraki-Video-Editor.git
cd Meraki-Video-Editor
bash scripts/setup-media.sh
cargo test --workspace --locked
cd apps/desktop
npm ci
npm test
npm run dev
```

---

## 🧱 Estrutura dos Crates do Workspace

| Crate | Responsabilidade |
|---|---|
| `meraki-core` | Aritmética racional de tempo, taxas de quadros, timecode SMPTE e identificadores tipados. |
| `meraki-timeline` | Trilhas de áudio/vídeo, clipes, mapeamento de fontes, trim, marcadores e keyframes. |
| `meraki-graph` | Grafo acíclico dirigido de efeitos (DAG), verificação de ciclos e ordenação topológica. |
| `meraki-engine` | Máquina de estados transacional, snapshots imutáveis e histórico de Undo monotônico. |
| `meraki-media` | Probe de mídia, catálogo de assets, decodificação, ring buffer de frames e proxies. |
| `meraki-audio` | Motor CPAL em tempo real, buffer SPSC, mixagem, faders, medidores VU e EQ biquad. |
| `meraki-compositor` | Composição em GPU via WGSL e CPU, efeitos de cor/desfoque e rasterização de texto. |
| `meraki-project` | Serialização `.meraki`, migrações de esquema, relink de mídia e journal de recuperação. |
| `meraki-render` | Iteração determinística de quadros, render queue e codificação sequencial FFmpeg. |
| `meraki-desktop` | Aplicação desktop Tauri v2, comandos IPC, roteamento de transporte e ciclo de vida. |

---

## 🤝 Comunidade e Contribuição

Contribuições são muito bem-vindas! Consulte o arquivo [CONTRIBUTING.md](CONTRIBUTING.md) para diretrizes de estilo de código, testes e fluxo de Pull Requests. Siga também nosso [Código de Conduta](CODE_OF_CONDUCT.md).

## 📄 Licença

O código-fonte do Meraki é licenciado sob **MIT OR Apache-2.0**, à sua escolha. Consulte os arquivos [LICENSE-MIT](LICENSE-MIT) e [LICENSE-APACHE](LICENSE-APACHE).
