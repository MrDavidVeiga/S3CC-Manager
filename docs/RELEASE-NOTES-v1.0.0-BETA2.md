# Veiga's S3CC Manager — v1.0.0 Beta 2

Mudanças em relação à Beta 1.

## Adicionado
- Reconhecimento de pacotes NRaas por assinaturas internas, além de poses e animações identificadas por CLIP e listas de poses.
- Novos critérios funcionais de classificação Build/Buy a partir dos recursos OBJD.
- Identificação conservadora de criador, nome do mod e categoria de jogabilidade, com nível de confiança e evidências consultáveis.
- Recuperação de miniaturas ausentes de Sims3Pack a partir dos caches do The Sims 3, sem substituir miniaturas já existentes.
- Ampliação do sistema de temas, interface Fluent/WinUI 3 e dados técnicos de diagnóstico.

## Removido
- Organização automática de packages desconhecidos, mistos, inválidos ou que ainda exigem revisão.
- Relocação automática de duplicatas e colisões sem resolução segura e explícita.
- Atribuição de criador ou identidade com base apenas em nomes de arquivos.

## Corrigido
- Exibição de CDATA em nomes localizados de Sims3Pack.
- Preservação e reconstrução de dados _KEY/NMAP em conversões combinadas.
- Falsas classificações de determinados scripts S3SA como CAS ou objetos comuns.
- Tratamento de destinos com colisões, caminhos fora das regras de carregamento ou risco de sobrescrita.

## Melhorado
- Planejamento de organização parcial, com itens incertos mantidos no local original e apenas movimentos executáveis no manifesto.
- Interpretação das regras PackedFile do Resource.cfg e compatibilidade da profundidade das pastas com o jogo.
- Prévias Antes → Depois, evidências de classificação, confirmações e proteção de operações reversíveis.
- Desempenho e responsividade das listas, pesquisas, filtros, caches e indicadores de progresso.
- Localização em português, inglês e espanhol e consistência visual dos controles e temas.
- Informações apresentadas nos relatórios e maior confiabilidade da conversão Sims3Pack → Package.
