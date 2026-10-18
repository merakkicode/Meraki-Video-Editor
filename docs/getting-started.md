# Seu primeiro projeto

## Importar e montar

Em **Montagem**, clique em **Importar**, escolha um vídeo e insira-o na linha do tempo pelo Acervo. V1 e V2 recebem vídeo; as trilhas de áudio exibem suas formas de onda. Use a agulha para navegar e **Espaço** para reproduzir ou pausar.

Selecione um clipe e pressione **Ctrl+B** para cortar na posição da agulha. Arraste o clipe para movê-lo. Arraste suas bordas para aparar, respeitando a duração disponível da fonte. Segure **Shift** antes de aparar para aplicar Ripple naquela trilha. **Ctrl+Z** desfaz a operação.

**N** liga ou desliga o Encaixe. Ao aproximar a agulha, um clipe ou uma borda de outro corte ou marcador, a guia ciano mostra o ponto de encaixe. **M** cria um marcador; o duplo-clique permite editar sua anotação e cor.

## Imagem, títulos e som

Em **Ajustes**, altere escala, posição e opacidade. Use o diamante da propriedade para criar keyframes na posição atual e animar o clipe. A ferramenta **Texto** cria um título com fundo transparente na trilha superior.

Em **Trama**, selecione o clipe e ajuste cor ou desfoque. Bypass desativa o processamento daquele nó. Para Picture-in-Picture, sobreponha um clipe na trilha superior, reduza sua escala e ajuste sua posição.

Em **Som**, os faders controlam ganho, **M** nos controles da trilha silencia e **S** isola. Ajuste o equalizador e acompanhe os medidores, evitando níveis de pico excessivos. A tecla de marcador **M** é um atalho da timeline; botões M/S da Mesa são controles de áudio.

## Salvar e recuperar

Salve em `.meraki` pelo comando de salvar projeto da interface. O documento guarda a edição e referências às mídias, sem copiar os vídeos para dentro dele. Se um arquivo mudar de lugar, use **Reconectar Mídia** no menu de contexto.

O autosave cria um checkpoint a cada três minutos. Se houver uma recuperação disponível ao iniciar, revise-a e salve o projeto. Edições feitas após o último checkpoint podem não estar presentes.

Para mídia pesada, use **Gerar Proxy (720p)** no Acervo e ative **Usar Proxies**. Isso altera a fonte usada na prévia; a exportação sempre consulta os originais.

## Exportar

1. Abra **Entrega** e escolha um preset, como **YouTube — Qualidade Superior**.
2. Informe nome e destino do arquivo. Confira resolução, taxa de quadros e intervalo.
3. Clique em **Adicionar à Fila de Render** e depois em **Iniciar Render**.
4. Aguarde **Concluído** e abra o vídeo exportado.

O arquivo MP4 H.264 é a opção mais simples para compartilhamento. H.265 e ProRes dependem de suporte no reprodutor de destino. A fila mostra progresso, FPS e estimativa de tempo; o cancelamento remove a saída temporária incompleta.

## Atalhos de edição

| Atalho | Ação |
|---|---|
| Espaço | Reproduzir / pausar |
| Ctrl+B | Cortar o clipe selecionado |
| Ctrl+Z | Desfazer |
| V | Seleção |
| C | Corte |
| N | Encaixe |
| M | Marcador |

Os atalhos editoriais não devem substituir a digitação em campos de texto. Consulte [Validação e limites](testing.md) e [Suporte](https://github.com/merakkicode/Meraki-Video-Editor/issues/new/choose).
