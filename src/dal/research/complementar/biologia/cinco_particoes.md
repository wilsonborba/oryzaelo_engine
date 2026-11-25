# Seis estádios nas cinco partições originais do TCC

Comparação retrospectiva com todas as 2.370 vistorias tailandesas de 2023, 2024 e 2025. Cada vistoria foi avaliada uma vez, usando para treino as outras quatro partições tailandesas e, quando indicado, a amostra externa fixa do benchmark. As configurações de referência, interação com fotoperíodo e perfil foliar são as mesmas da medição biológica temporal. A V2 usa CatBoost e o componente de vistorias semelhantes. O XGBoost emprega a codificação, a semente por partição e os hiperparâmetros originais. As 2.370 previsões do XGBoost de referência coincidiram individualmente com o arquivo histórico.

| Modelo | Referência | Interação cultivar × duração do dia | Interação e perfil foliar |
|---|---:|---:|---:|
| XGBoost original, Tailândia | 0,6189 | 0,6133 | 0,6153 |
| V2 combinada, Tailândia | 0,6059 | 0,6076 | 0,6120 |
| V2 combinada, Tailândia + Espanha + EUA + Japão | 0,6071 | 0,6083 | 0,6140 |

O macro-F1 nas cinco partições responde à pergunta principal do projeto sobre classificação das seis fases na distribuição histórica. Talhões próximos e vistorias de uma mesma safra podem aparecer no treino e na avaliação. O teste temporal adicional reserva todas as 856 vistorias de 2025 para avaliar o desempenho em uma safra posterior. As duas avaliações respondem a perguntas distintas. A avaliação original suplementar de seis estádios com dados adicionais da própria vistoria obteve 0,6634. A avaliação de três macrofases obteve 0,7609 e usa rótulos agrupados.

[Métricas](../../../data/tcc/complementar/biologia/cinco_particoes_metricas.csv) · [Previsões individuais](../../../data/tcc/complementar/biologia/cinco_particoes_predicoes.csv.gz) · [Código da reprodução](cinco_particoes.py) · [Avaliação temporal](resultados.md)
