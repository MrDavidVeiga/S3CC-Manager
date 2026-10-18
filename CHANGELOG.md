# Veiga's S3CC Manager — Changelog

## v1.0.0 Beta 2 (em comparação com a Beta 1)

### Adicionado
- Reconhecimento por assinaturas internas de pacotes NRaas, poses e animações por CLIP e listas de poses.
- Metadados de criador, nome do mod, categoria de jogabilidade e nível de confiança da classificação, com evidências na prévia e nos relatórios.
- Classificação funcional mais detalhada de objetos Build/Buy a partir de OBJD.
- Recuperação opcional de miniaturas ausentes de Sims3Pack nos caches do The Sims 3, preservando as existentes.
- Temas ampliados e interface Fluent inspirada no WinUI 3, com informações de diagnóstico mais completas.

### Removido
- Movimentação automática de pacotes desconhecidos, mistos, inválidos ou sem evidência suficiente para uma categoria confiável.
- Encaminhamento automático de casos não resolvidos de duplicidade ou colisão para pastas de destino arbitrárias.
- Dependência do nome do arquivo como prova isolada de identidade ou criador.

### Corrigido
- Nomes localizados de Sims3Pack com marcações CDATA exibidas incorretamente.
- Preservação e reconstrução de informações de nomes _KEY/NMAP na conversão combinada.
- Classificações indevidas de scripts S3SA como objetos ou conteúdo CAS comum.
- Validação de destinos sujeitos a colisões, sobrescrita ou caminhos não carregáveis pelo Resource.cfg.

### Melhorado
- Organização parcial: arquivos corretamente identificados podem ser organizados, enquanto os que precisam de revisão permanecem onde estão.
- Planejamento Antes → Depois, verificação de colisões, geração de manifesto e informações de classificação.
- Tratamento das regras PackedFile e da profundidade de pastas para preservar o carregamento dos packages.
- Resposta das listas, pesquisa, filtros, reutilização do cache e acompanhamento do progresso.
- Localização PT/EN/ES, comportamento dos temas, relatórios e diagnósticos.
- Identificação de nomes e preservação de miniaturas na conversão Sims3Pack → Package.

## v1.0.0 Beta 1

Veja [Notas de lançamento da Beta 1](docs/RELEASE-NOTES-v1.0.0-BETA1.md).
