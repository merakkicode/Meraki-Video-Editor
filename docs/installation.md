# Instalar o Meraki

Baixe somente pela [release oficial](https://github.com/merakkicode/Meraki-Video-Editor/releases/tag/v1.0.0). Há um pacote de uso para cada sistema: **Meraki-Setup.exe** e **Meraki.AppImage**. Fontes e checksums são materiais complementares, sem necessidade de instalação.

## Windows 10 e 11 — x64

1. Baixe `Meraki-Setup.exe` e abra o arquivo.
2. Selecione o idioma do instalador e conclua a instalação para seu usuário.
3. Abra **Meraki Video Editor** pelo Menu Iniciar.

O instalador inclui o runtime de mídia. Caso necessário, baixa o Microsoft WebView2; essa etapa precisa de internet. O pacote ainda não possui assinatura digital e o Windows pode apresentar um aviso de editor desconhecido. Confira a origem e o SHA-256 antes de decidir executá-lo; não desative a proteção do sistema.

Para remover o programa, use **Configurações → Aplicativos → Meraki Video Editor → Desinstalar**. Guarde seus projetos e arquivos de mídia em uma pasta própria.

## Linux — x64

O AppImage é um aplicativo portátil. A base de compilação é Ubuntu 22.04 (glibc 2.35). Use uma sessão gráfica, drivers de GPU atualizados e um dispositivo de áudio disponível via ALSA ou compatibilidade PulseAudio/PipeWire. Isso não representa certificação de todas as distribuições.

```bash
chmod +x Meraki.AppImage
./Meraki.AppImage
```

Também é possível usar **Propriedades → Permissões → Permitir executar como programa** no gerenciador de arquivos.

Se aparecer um erro de FUSE:

```bash
./Meraki.AppImage --appimage-extract-and-run
```

Se faltar saída de áudio, confira o dispositivo padrão e os plugins ALSA da distribuição. Em Ubuntu/Debian, o pacote é `libasound2-plugins`. A aceleração depende do driver Vulkan instalado. Falhas de GPU podem acionar o compositor em CPU.

Para remover o aplicativo portátil, exclua o AppImage. Seus projetos ficam nos locais escolhidos ao salvar. Flatpak, Flathub, MSI e DEB não são canais oficiais desta release.

## Verificar o download

Baixe `SHA256SUMS` na mesma release e compare a linha correspondente ao arquivo:

```powershell
Get-FileHash .\Meraki-Setup.exe -Algorithm SHA256
```

```bash
sha256sum Meraki.AppImage
```

O hash confirma a integridade em relação ao arquivo publicado. Ele não substitui assinatura digital.

## Atualizar ou pedir ajuda

Salve o projeto e feche o editor antes de atualizar. Faça uma cópia de segurança dos projetos importantes. No Windows, execute o novo instalador; no Linux, substitua o AppImage depois de verificar o download.

Ao [relatar um problema](https://github.com/merakkicode/Meraki-Video-Editor/issues/new/choose), inclua versão do Meraki, sistema, placa gráfica e passos para reproduzir. Não envie mídia privada. Veja também [Primeiros passos](getting-started.md).
