# Protocolo da medição biológica V2

Definido antes de calcular os resultados desta extensão, em 25/11/2026. A safra de 2025 já foi avaliada no benchmark anterior. Ela serve para uma comparação temporal retrospectiva. Uma validação prospectiva requer uma safra posterior reservada.

**Pergunta primária.** A interação entre a classificação oficial da sensibilidade ao fotoperíodo e a duração do dia melhora o macro-F1 das seis classes no candidato internacional existente? O contraste primário compara o braço interação com a referência no treino TH + ES + US + JP. O treino exclusivamente tailandês é uma replicação do contraste com outra composição. A meta continua sendo 0,75.

**Experimentos fixos.** Usar os mesmos 44 preditores originais com as mesmas janelas NASA POWER, CatBoost, hiperparâmetros, ponderação de classes e países e componente de vistorias semelhantes (50%, dez dias e escala espacial de 0,45°). Acrescentar variáveis apenas ao CatBoost. Manter as categorias e os nomes originais do componente de similaridade.

| Braço | Acréscimo à referência | O que testa |
|---|---|---|
| referencia | Nenhum | Reprodução ponto a ponto das previsões históricas |
| cobertura | Indicador de que há ficha oficial no dicionário fixo | Efeito de identificar uma cultivar com ficha |
| fotoperiodo | Cobertura e indicador de sensibilidade | Sensibilidade oficial além da presença da ficha |
| interacao | Braço anterior + sensibilidade × (horas de luz − 12) | Hipótese primária. A duração do dia já está na referência |
| identidade | Interação + três indicadores de identidade das cultivares do estudo de nutrição | Controle exploratório para o reconhecimento das três cultivares |
| perfil_foliar | Interação + presença de perfil e N, P e K típicos da folha no máximo perfilhamento | Exploração de um perfil fixo de referência por cultivar. Cada cultivar recebe sempre os mesmos três valores em qualquer data ou estádio |

**Fontes admissíveis.** Usar as quatro equivalências previamente documentadas, com normalização exclusiva de espaços. Restringir o vínculo aos registros tailandeses. Uma equivalência internacional exigiria identidade varietal confirmada. Ausência de ficha é representada explicitamente. No perfil foliar, os valores vêm das tabelas 1 a 3 de Kasetsart/Naresuan. A consulta usa somente a identidade da cultivar. Nem a classe da vistoria nem medições da parcela feitas posteriormente participam da consulta. Três perfis fixos têm poder limitado para distinguir identidade de mecanismo biológico. Temperatura de ensaios controlados, CO₂ experimental e medições em grãos colhidos ficam na discussão científica.

**Avaliação.** Reusar os três folds geográficos publicados de 2023–2024 e as mesmas 856 vistorias tailandesas de 2025. Calcular macro-F1 fixando seis classes, acurácia, métricas por classe, matriz de confusão e cobertura dos traços. Calcular diferença pareada de macro-F1 com 5.000 reamostragens das células geográficas de 0,1° de 2025, usando a mesma amostra para ambos os modelos. Informar as réplicas com as seis classes. A unidade é a célula aproximada, pois o arquivo não identifica todas as fazendas. A incerteza é condicional aos modelos ajustados e à safra disponível.

**Decisão.** Relatar todos os braços. A interpretação primária se apoia no sinal do contraste predefinido em desenvolvimento e em 2025 e em seu intervalo pareado. Não escolher uma especificação pelo maior valor de 2025. A biologia pode entrar na discussão da parcial independentemente do resultado preditivo. Promoção do modelo e medição no Raspberry Pi são decisões posteriores.
