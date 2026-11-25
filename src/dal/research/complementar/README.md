# Pesquisa complementar

## Resultado de 25 de novembro de 2026

| Composição do treino | Macro-F1 de desenvolvimento do CatBoost | Macro-F1 do CatBoost em 2025 | Macro-F1 do modelo combinado em 2025 |
|---|---:|---:|---:|
| Tailândia | 0,487 | 0,275 | 0,331 |
| Tailândia + Espanha | 0,498 | 0,304 | 0,334 |
| Tailândia + EUA + Japão | 0,496 | 0,263 | — |
| Quatro países | 0,491 | 0,251 | 0,339 |

O desenvolvimento abrange 1.514 observações tailandesas até dezembro de 2024. O teste compreende as mesmas 856 vistorias de janeiro a setembro de 2025 em todas as linhas. Cada grupo geográfico de desenvolvimento pertence a uma única partição, calculada em malha de 0,1° como aproximação à localização de visitas próximas. Há 429 grupos no desenvolvimento e 231 no teste. A fronteira da malha pode separar parcelas vizinhas. O agrupamento climático de 0,5° serve para consultar a NASA e tem finalidade distinta.

O XGBoost histórico chegou a 0,619 nas partições aleatórias originais e a 0,241 quando a mesma especificação foi reajustada ao corte temporal. O CatBoost experimental obteve 0,593 com a Tailândia e 0,592 com os quatro países nas mesmas cinco partições aleatórias do TCC. Nas cinco partições por província, os resultados foram 0,380 e 0,375. O Macro-F1 de 0,761 do estudo anterior corresponde a três macrofases. A meta de seis estádios segue em 0,75.

Na primeira comparação, a contribuição espanhola estimada foi +0,030 (intervalo de 95% de −0,021 a +0,094). Depois desse teste explorou-se uma mistura de CatBoost e vistorias de calendário, região e clima semelhantes. Os parâmetros vieram da validação de desenvolvimento. O combinado de quatro países atingiu 0,339, com intervalo de 0,287 a 0,390. A diferença pareada sobre o combinado tailandês foi +0,008 (−0,005 a +0,021). O estudo anterior já incluía as observações de 2025. Uma afirmação confirmatória depende de uma nova coleta. Os intervalos reamostram os locais testados com os modelos fixos e não abrangem a variação de uma safra futura.

O combinado de quatro países acertou 295 de 389 casos de perfilhamento, 3 de 17 casos de floração e 0 de 65 casos de pré-colheita. Entre os casos de pré-colheita, 29 foram atribuídos a emborrachamento e 23 a espigamento. Nenhuma fonte externa forneceu pré-colheita equivalente. O desempenho em todas as fases, sobretudo dentro do mesmo campo ao longo da safra, é o critério para decidir sobre uma futura aplicação. O estudo usa clima de dia completo até a data da vistoria. Uma aplicação durante o próprio dia exige um protocolo separado para os dados então disponíveis.

## Fontes e tratamento

| Base | Observações aceitas com clima | Observações externas usadas no treino | Proveniência e limites |
|---|---:|---:|---|
| Tailândia | 2.370 | — | Levantamento de pragas e doenças do Rice Department com estádio registrado em campo, 2.398 antes da limpeza. Os arquivos estão no [catálogo de dados](../../data/README.md). |
| Espanha | 18.558 | 1.600 | RAIF, Junta de Andalucía, dois arquivos XML por tipo de entidade na distribuição. 1.019 parcelas na amostra aceita, 572 datas. Cinco estádios, sem pré-colheita. [Catálogo](https://www.juntadeandalucia.es/datosabiertos/portal/dataset/raif) e [protocolo de campo](https://www.juntadeandalucia.es/agriculturapescaaguaydesarrollorural/raif/wp-content/uploads/2023/07/ProtocolosCampos_Arroz.pdf). |
| Estados Unidos | 432 | 139 | Parcelas de Arkansas, 55 datas. Calculou-se 50% de espigamento a partir da emergência de cada parcela. Os CSV vieram do repositório dos autores, commit 2e9a01bee7d2c604ef16e7ed1c56361e7de5406f. Usou-se o centro do ponto de consulta 34,5° N, 91,5° W para representar a estação. [Dryad](https://doi.org/10.5061/dryad.v41ns1s4z), [dados agronômicos](https://github.com/FareedFarag/TPPJ-Modeling-Code/tree/2e9a01bee7d2c604ef16e7ed1c56361e7de5406f/Hypothesis_2/Datasets/Agronomic) e [cadastro da estação](https://glis.fao.org/glis/wiews/detail?wiewsCode=USA970). |
| Japão | 6 | 6 | Três cultivares em uma estação de Kuroishi, 2023–2024. Datas de espigamento transcritas das colunas observadas 本年 e 前年. O geocodificador do GSI localizou o bairro Tanaka em 40,665489° N e 140,588745° E. A posição representa a localidade da estação. [Boletim primário](https://www.nounavi-aomori.jp/farmer/wp-content/uploads/2024/08/4b55ec8b241fd9b0b9eff0bec315f5c6-1.pdf) e [endereço](https://aomori-itc.or.jp/aboutcenter/access.html). |

A extração espanhola restringiu-se a 2015–2024, Sevilha e Cádiz, à faixa espacial definida em `scripts/spain.py` e a uma coordenada/variedade única por parcela-campanha. Os XML identificam as coordenadas como UTM. O procedimento testou dois datums plausíveis, ETRS89 e ED50 no fuso 30, e exigiu a mesma célula de consulta nas duas conversões. A posição exata da parcela continua condicionada ao datum. A redação do protocolo e a presença de um único estádio positivo apoiam o mapeamento conservador do valor 2. A codificação por campanha e a correspondência com a vistoria tailandesa ainda comportam incerteza. O arquivo de auditoria distingue exclusões por linha e por visita.

O serviço POWER forneceu as temperaturas máxima, mínima e média a 2 m, chuva corrigida, radiação de onda curta e umidade relativa. Para cada combinação de local e data, as janelas inclusivas de 7, 14, 30 e 60 dias terminam na vistoria. A radiação usa MJ/m²/dia e a chuva usa mm/dia. Verificaram-se fuso solar local, unidades, coordenadas, dias ausentes e sentinelas em cada resposta. As 18.996 observações internacionais (75.984 janelas) passaram pela verificação. O teste de equivalência em 40 linhas tailandesas encontrou diferença máxima de 1,78 × 10⁻¹⁵ nos atributos climáticos e de calendário. As URLs e hashes das sete consultas externas estão em `results/weather_manifest.json`. [Documentação diária da NASA POWER](https://power.larc.nasa.gov/docs/services/api/temporal/daily/).

A amostragem antes do treino reteve uma linha por país, célula, data, classe e variedade, e até 320 por país/classe. O total de peso externo de cada classe ficou limitado a 30% do seu peso tailandês. Esse limite reduz a influência de campanhas densamente amostradas. Os três registros japoneses por safra e a única classe americana dão pouca informação sobre a discriminação entre estádios.


## Reproduzir as comparações

Instale o ambiente seguindo o [guia principal](../README.md). Execute na raiz do repositório:

```bash
python src/dal/research/complementar/reproduce.py audit
python src/dal/research/complementar/reproduce.py train
```

A auditoria recalcula os resultados do combinado, as comparações biológicas por cobertura e as métricas das quatro arquiteturas nas três avaliações. O treino repete a seleção por validação, o corte temporal, as análises de sensibilidade e os experimentos de biologia. Também verifica as classes do candidato exportado nas 856 vistorias de 2025. As saídas ficam em `src/dal/data/local/reproduction/complementar-audit/` e `src/dal/data/local/reproduction/complementar-train/`.

| Pergunta | Texto | Evidência por observação |
|---|---|---|
| Acrescentar outros países melhora a previsão na safra de 2025? | Métodos e resultados acima | [Previsões por composição](../../data/processed/complementar/results/forward_predictions.csv.gz) |
| A informação biológica da cultivar agrega valor? | [Pesquisa](biologia/pesquisa.md), [protocolo](biologia/protocolo.md), [resultados temporais](biologia/resultados.md), [cinco partições](biologia/cinco_particoes.md) | [Previsões e contrastes](../../data/processed/complementar/biologia) |
| As arquiteturas recentes superam o controle? | [Pesquisa](modelos_recentes/pesquisa.md) e [protocolo](modelos_recentes/protocolo.md) | [Previsões e métricas por modelo](../../data/processed/complementar/modelos_recentes) |

## Reconstruir as fontes internacionais

A matriz climática publicada permite iniciar o treino diretamente. Para repetir também a extração, use PROJ (`cs2cs`), Poppler (`pdftotext`) e a biblioteca Python `lxml`. Os comandos trabalham numa cópia dos insumos. O boletim de Aomori e as fontes biológicas são obtidos dos provedores quando necessário.

```bash
python -m pip install lxml
mkdir -p src/dal/data/local/reproduction/extracao
cp -R src/dal/data/raw/complementar/. src/dal/data/local/reproduction/extracao/
export ORYZELO_SOURCES="$PWD/src/dal/data/local/reproduction/extracao"
export ORYZELO_OBSERVATIONS="$PWD/src/dal/data/local/reproduction/observacoes"
export ORYZELO_RESULTS="$PWD/src/dal/data/local/reproduction/extracao-resultados"
mkdir -p "$ORYZELO_RESULTS/results" "$ORYZELO_OBSERVATIONS"
python src/dal/research/complementar/scripts/spain.py
python src/dal/research/complementar/scripts/trials.py
python src/dal/research/complementar/scripts/weather.py
unset ORYZELO_SOURCES ORYZELO_OBSERVATIONS ORYZELO_RESULTS
python src/dal/research/complementar/biologia/extrair.py
```

## Retreinar as arquiteturas neurais

As previsões de referência permitem auditar os resultados em CPU. Para repetir os ajustes, o ambiente do estudo utilizou Python 3.12 em Linux, uma GPU NVIDIA compatível com CUDA 12.4 e PyTorch 2.6.0. Em um terminal separado, na raiz do repositório:

```bash
python3.12 -m venv src/dal/data/local/reproduction/neural/.venv
. src/dal/data/local/reproduction/neural/.venv/bin/activate
python -m pip install 'torch==2.6.0' --index-url https://download.pytorch.org/whl/cu124
python -m pip install 'tabicl==2.2.0' 'tabpfn==10.2.0' 'tabm==0.0.3' 'numpy==2.5.5' 'pandas==2.3.3' 'scikit-learn==1.7.2'
python src/dal/research/complementar/modelos_recentes/checkpoints.py
export ORYZELO_RESULTS="$PWD/src/dal/data/local/reproduction/neural"
for model in tabm tabicl tabpfn35; do
  python src/dal/research/complementar/modelos_recentes/avaliar.py "$model" || exit 1
done
```

O [manifesto dos pesos](modelos_recentes/checkpoints.json) fixa a revisão, o tamanho e o SHA-256. O provedor do TabPFN pode exigir aceite dos termos e um token de leitura via `HF_TOKEN`. As previsões saem em `src/dal/data/local/reproduction/neural/modelos_recentes/`. O XGBoost de controle usa o ambiente CPU do estudo. Para confrontar os novos resultados, copie os arquivos de referência `xgb_predicoes.csv.gz` e `results/archived_spec_forward.csv` para as pastas correspondentes em `src/dal/data/local/reproduction/neural/` e execute `modelos_recentes/resumir.py` a partir da pasta deste guia, no ambiente CPU com o mesmo `ORYZELO_RESULTS`.
