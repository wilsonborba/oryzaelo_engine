# Dados do TCC

Os caminhos desta tabela são relativos a esta pasta. [Instruções de instalação e reprodução](../../research/README.md).

| Dados ou evidência | Arquivo | Origem e uso |
|---|---|---|
| Vistorias tailandesas | [CSV bruto](../raw/ricepest_survey_combined_66_68.csv) | Rice Department, Tailândia. 2.398 linhas antes da limpeza. O identificador analítico corresponde à posição da linha de dados, a partir de zero. |
| Meteorologia tailandesa | [124 respostas por célula](../raw/nasa_power_cache) | NASA POWER, dados diários. As coordenadas consultadas e as datas estão nos arquivos. |
| Matriz tailandesa analisada | [features.csv](principal/features.csv), [manifesto de atributos](principal/experiment_manifest.json) | 2.370 observações. O manifesto identifica as 44 colunas preditoras e suas unidades. |
| Divisões de treino e teste | [cinco partições estratificadas](principal/six_stage_folds.csv), [por província](principal/province_folds.csv) | Associar por `source_row_id`. As previsões fora da amostra e as métricas ficam na mesma pasta. |
| Inferência no Pi | [modelo e entrada](principal), [durações por rodada](principal/pi_benchmark) | Exportação do CatBoost numérico em seis classes e medições históricas do mesmo artefato. |
| Vistorias espanholas | [fonte RAIF e extrações](complementar/fontes) | Junta de Andalucía, Servicio de Sanidad Vegetal, CC BY 4.0. O ZIP original reúne XML de parcelas e vistorias. O CSV derivado filtra local, anos e estádio da visita. [Catálogo](https://www.juntadeandalucia.es/datosabiertos/portal/dataset/raif) |
| Espigamento em Arkansas e Aomori | [trials.csv](complementar/fontes/trials.csv) | Datas de emergência e intervalos das parcelas americanas, mais seis datas transcritas do boletim japonês. [Dados dos autores](https://doi.org/10.5061/dryad.v41ns1s4z) · [Boletim de Kuroishi](https://www.nounavi-aomori.jp/farmer/wp-content/uploads/2024/08/4b55ec8b241fd9b0b9eff0bec315f5c6-1.pdf) |
| Meteorologia e atributos internacionais | [respostas NASA](complementar/fontes/power), [matriz internacional](complementar/fontes/international_features.csv.gz) | 18.558 registros espanhóis, 432 americanos e seis japoneses com clima. O subconjunto de treino contém 1.600, 139 e seis registros, respectivamente. |
| Biologia do arroz | [tabelas e manifesto de fontes](complementar/fontes/biologia) | Sensibilidade ao fotoperíodo, N/P/K foliar e análises descritivas de ensaios. A [pesquisa biológica](../../research/complementar/biologia/pesquisa.md) explica a utilização de cada fonte. |
| Resultados complementares | [países](complementar/results), [biologia](complementar/biologia), [modelos recentes](complementar/modelos_recentes) | Previsões por observação, partições de desenvolvimento e tabelas por protocolo. |

As fontes conservam as condições de uso de seus provedores. As assinaturas em [SHA256SUMS](SHA256SUMS) cobrem os dados históricos, as previsões e as respostas da NASA desta versão.
