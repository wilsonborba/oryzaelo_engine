# Medição dos dados biológicos no Oryzelo V2

O perfil foliar elevou o macro-F1 retrospectivo de 0,3313 para 0,3605 no treino tailandês e de 0,3389 para 0,3555 no internacional. A hipótese primária sobre sensibilidade ao fotoperíodo × duração do dia teve resultado de 0,3341 contra 0,3389 no internacional. A incerteza e a concentração espacial dos acertos sustentam manter o modelo de referência como candidato e apresentar o perfil foliar como análise exploratória na parcial.

## Desenho

Foram 48 ajustes de CatBoost, combinando dois conjuntos de treino, seis especificações e quatro ajustes de cada especificação (três folds geográficos e um treino com todo o desenvolvimento). Todos usaram as mesmas janelas NASA POWER, pesos e componente de vistorias semelhantes. A referência reproduziu as previsões históricas linha por linha nas 856 vistorias de 2025, em ambos os conjuntos de treino. Os escores de desenvolvimento também coincidiram com os publicados. Cada linha de desenvolvimento recebeu uma previsão de um modelo treinado em outras células. O período 2023–2024 teve 1.514 vistorias e 429 células, e o teste histórico incluiu 231 células. O limiar de sucesso continua em macro-F1 de 0,75 para seis estádios.

Dos 1.514 registros de desenvolvimento, 281 tinham fotoperíodo documentado e 182 tinham perfil foliar. Em 2025, a cobertura foi de 182/856 (21,3%) para fotoperíodo e 116/856 (13,6%) para o perfil. O perfil usa N, P e K da literatura no máximo perfilhamento como atributo fixo de cada uma das três cultivares. Uma consulta sempre devolve os mesmos três valores, qualquer que seja a fase da vistoria. A duração do dia e o nome original da cultivar já integravam todos os controles. [Protocolo](protocolo.md) · [Pesquisa e fontes](pesquisa.md)

## Macro-F1 das seis classes

| Preditores adicionais | Tailândia, desenvolvimento | Tailândia, 2025 | Internacional, desenvolvimento | Internacional, 2025 |
|---|---:|---:|---:|---:|
| Referência | 0,4912 | 0,3313 | 0,5043 | 0,3389 |
| Cobertura da ficha | 0,4985 | 0,3454 | 0,5016 | 0,3139 |
| Cobertura e sensibilidade | 0,4881 | 0,3317 | 0,5027 | 0,3411 |
| Interação com duração do dia | 0,5030 | 0,3228 | 0,4984 | 0,3341 |
| Interação e identidade das três cultivares do estudo nutricional | 0,4904 | 0,3399 | 0,4915 | 0,3219 |
| Interação e perfil foliar fixo | 0,4930 | 0,3605 | 0,4941 | 0,3555 |

O treino internacional usa Tailândia, Espanha, EUA e Japão. Todas as avaliações da tabela usam vistorias tailandesas. Desenvolvimento é o macro-F1 calculado sobre as previsões agrupadas dos três folds, com cada vistoria presente uma vez. O teste de 2025 já havia sido consultado no benchmark anterior. Ele mede a repetição temporal retrospectiva e a comparação dos braços sobre as mesmas vistorias. [Todas as métricas, incluindo os folds](../../../data/processed/complementar/biologia/metricas.csv)

## Incerteza e localização dos acertos

| Comparação com a referência no teste de 2025 | Diferença | IC 95% pareado por célula |
|---|---:|---:|
| Internacional, interação | −0,0049 | −0,0250 a +0,0103 |
| Tailândia, interação | −0,0085 | −0,0202 a +0,0035 |
| Internacional, perfil foliar | +0,0166 | −0,0258 a +0,0517 |
| Tailândia, perfil foliar | +0,0292 | −0,0128 a +0,0656 |

As 5.000 réplicas de bootstrap geográfico continham as seis classes. O procedimento reaproveita exatamente o mesmo sorteio de células para cada modelo. Os intervalos expressam a variação entre células desta amostra, condicional ao ajuste realizado. Os contrastes secundários são exploratórios, com intervalos nominais sem ajuste por comparações múltiplas. [Contrastes completos](../../../data/processed/complementar/biologia/contrastes_2025.csv)

**Pré-colheita.** O perfil foliar acertou 6 das 65 vistorias de pré-colheita, contra zero na referência, em ambos os conjuntos de treino. Os seis acertos ocorreram em 11/06/2025 na mesma célula aproximada, todos em vistorias sem perfil foliar identificado. Removendo essa célula inteira na análise de sensibilidade posterior à medição, a diferença do perfil internacional passa de +0,0166 para −0,0066, e a do treino tailandês passa de +0,0292 para +0,0063. Acrescentar variáveis muda as árvores ajustadas e pode alterar as previsões até de cultivares sem ficha. A contagem de acertos, portanto, não permite atribuir o resultado à nutrição da planta. [Métricas por classe](../../../data/processed/complementar/biologia/por_classe.csv) · [Influência ao omitir cada célula](../../../data/processed/complementar/biologia/influencia_celulas.csv)

**Cultivares cobertas.** Nas 116 vistorias com perfil foliar, o macro-F1 de seis classes do candidato internacional passou de 0,2308 para 0,1928. Nas outras 740 passou de 0,3498 para 0,3737. A acurácia internacional agregada foi 53,74% tanto na referência como no braço de perfil. A seleção das quatro cultivares foi informada pelo inventário de nomes na base completa. Os documentos varietais são externos, e o estudo dispõe de uma única safra posterior ao desenvolvimento. [Resultados por cobertura](../../../data/processed/complementar/biologia/por_cobertura.csv)

## Decisão para o TCC

A fisiologia das cultivares tem valor para a fundamentação e a discussão do trabalho tailandês. As métricas atuais sustentam descrever o perfil foliar como resultado preliminar da V2. O critério primário não mostrou melhoria temporal e o maior valor retrospectivo, 0,3605, continua aquém da meta. Para verificar generalização, a prioridade é avaliar em uma safra reservada prospectivamente, com pré-colheita distribuída por vários locais, e ampliar a cobertura varietal usando fontes verificáveis. Nenhum dos dois acréscimos justifica promover um modelo de produção nesta medição.

## Reprodução e auditoria

Consulte o [roteiro de reprodução](../README.md).
