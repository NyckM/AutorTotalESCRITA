# Autor Total Escrita

O editor de texto do [Autor Total](https://autortotal.com.br/) para Windows, Mac e Linux, com o **Mentor** ao lado
do documento.

- Abre e salva **.docx, .doc, .odt, .rtf, .md e .txt** do seu computador, com estilos, tabelas, revisão
  (controle de alterações), comentários, sumário e exportação em PDF.
- **Mentor** (F12 ou a aba ✦ na borda direita): entre com a conta do Autor Total e converse sobre o texto.
  Use a seleção, deixe o Mentor ler o documento, escolha o grau de crítica e insira a resposta no texto ou como
  comentário na margem. Usa a mesma cota de mensagens do app online.
- **Enviar para o Autor Total online**: cria uma história nova na sua conta com o texto do documento e abre o app
  no navegador, já na Escrita.

## Baixar

Os instaladores ficam na página [Releases](../../releases) deste repositório e em
https://autortotal.com.br/instalar.html.

## Compilar

Pela aba **Actions › Autor Total Escrita › Run workflow** (gera o instalador do Windows, o .dmg do Mac e o pacote
do Linux e publica uma versão). Localmente: `cargo build --release -p wordcraft` com `CRAFT_I18N_DIR` apontando
para o [craft-i18n](https://github.com/storytold/craft-i18n) na revisão de `craft-i18n.rev` (sem ele a interface
fica só em inglês).

O Mentor está em `crates/ui-egui/src/mentor.rs`. O programa só guarda neste computador o token de sessão da
pessoa (pasta AutorTotalEscrita nas configurações do usuário); nenhuma chave secreta fica no código.

## Licença e créditos

Baseado no [WordCraft](https://github.com/storytold/wordcraft) (MIT ou Apache-2.0, à sua escolha), dos autores do
WordCraft. O código continua sob as mesmas licenças (`LICENSE-MIT`, `LICENSE-APACHE`, `NOTICE`). O nome e os logos
do ArtCraft são marcas dos donos e não fazem parte deste programa. O README original está em
`README-WordCraft.md`.
