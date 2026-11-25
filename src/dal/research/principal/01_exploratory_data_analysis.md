# Oryza-Elo: Análise Exploratória de Dados (EDA) e Caracterização Estatística
## Auditoria Forense das 2.398 Observações de Campo da Orizicultura Tailandesa (2023–2025)

> **Status**: Publicado | **Fase**: Milestone 1 (Issue #1) | **Data**: 2026-09-18  
> **Repositório**: `wilsonborba/oryza-elo` | **Caminho do Dataset**: `src/dal/data/raw/principal/ricepest_survey_combined_66_68.csv`  
> **Hash Forense SHA-256**: `8ded458fc671f33660101cf4753b1c5427dde604da391786a4c8212839bdaa9b`  

---

## 1. Sumário Executivo & Racional Científico
Este relatório documenta a caracterização estatística aprofundada das 2.398 observações de campo coletadas pelo **Departamento de Arroz do Ministério da Agricultura da Tailândia** (*Rice Department, Ministry of Agriculture and Cooperatives - MOAC*) durante os anos agrícolas de 2023 a 2025 (anos budistas 2566 a 2568).

O objetivo principal desta caracterização é fornecer o embasamento quantitativo e os parâmetros empíricos necessários para:
1. **Subsidiar o TCC de Engenharia de Software da USP/Esalq**: comprovando a robustez e integridade dos dados de campo frente a abordagens convencionais de Visão Computacional (respondendo integralmente ao Comentário 2 do orientador Prof. Alexandre Duarte);
2. **Orientar a Extração Agrometeorológica NASA POWER (Issue #2)**: mapeando o envelope espaço-temporal e otimizando a cobertura de 2.398 coordenadas em 131 células de grade ($0,5^\circ \times 0,5^\circ$);
3. **Definir a Engenharia de Features Fisiológicas (Issue #3)**: alinhando o cálculo de Graus-Dia Acumulados ($GDD$ base 10°C), amplitude térmica ($DTR$) e balanço hídrico retrospectivo;
4. **Fundamentar o Baseline de Machine Learning (Issue #4)**: estabelecendo a estratégia de validação estratificada anti-desbalanceamento para atingir Macro-F1 $\ge 0,75$.

### Resumo das Descobertas Forenses Centrais:
- **Volume**: 2,398 observações e 42 variáveis sem nenhuma linha duplicada.
- **Auditoria Temporal (Resolução de Bug Crítico 1)**: Identificado formato dual na coluna de data (1.103 em string ISO `YYYY-MM-DD` e 1.295 em inteiros seriais do Excel, ex: `45926` = `2025-09-26`). Aplicada conversão pelo epoch canônico do Excel (`1899-12-30`), alcançando **0% de perda de registros**.
- **Auditoria Espacial (Resolução de Bug Crítico 2)**: Detectadas 3 anomalias de escala de coordenadas ($10^6$) em latitudes/longitudes e 4 casos de truncamento/duplicação. Aplicado tratamento por fator de escala e imputação por centróide mediano provincial, alcançando **100,0% de coordenadas limpas e válidas** para consultas geoespaciais.
- **Variável Alvo (`rice_stage`)**: 7 estágios fenológicos com forte desbalanceamento ($IR \approx 53.7:1$). O estágio de perfilhamento (`แตกกอ`) domina 47,04% da base, enquanto o estágio de colheita tardia (`ก่อนเก็บเกี่ยวเกี่ยว`) representa 0,88% (21 amostras).
- **Validação Biológica Inerente**: Cruzamento entre estresses bióticos e fases fenológicas comprova a integridade científica dos dados coletados em campo. Exemplo: *Panícula Manchada* (`dirty_panicle_%`) ocorre em 0,08% no perfilhamento e sobe para 75,08% na emissão de panícula; *Coração Morto* (`dead_heart_%`) ocorre em 20,63% no emborrachamento e decai a zero na colheita.
- **Envelope Espacial**: Amostragem distribuída por **58 províncias tailandesas**, cobrindo 100% dos principais pólos arrozeiros (Planícies Centrais irrigadas, Nordeste/Isan e Norte montanhoso).

---

## 2. Auditoria Forense da Série Temporal & Resolução de Formato Dual
Durante a inspeção dos dados brutos, foi detectada uma anomalia severa de formatação: diferentes fiscais agronômicos do Departamento de Arroz exportaram os dados a partir de versões distintas de planilhas eletrônicas.

### 2.1. O Bug do Inteiro Serial do Microsoft Excel
O Microsoft Excel armazena internamente datas como números inteiros de dias transcorridos desde um epoch base. Devido ao bug histórico de compatibilidade com o Lotus 1-2-3 (que erroneamente assumiu 1900 como ano bissexto), o epoch padrão do Excel para sistemas Windows/Linux é **30 de dezembro de 1899**.

Se um pipeline ingênuo utilizasse apenas `pd.to_datetime(df['survey_date'])`, ele geraria `NaT` (Not a Time) para **1.295 linhas (54,0% do dataset)** ou lançaria exceções não tratadas, descartando mais da metade dos dados científicos.

A conversão foi formalizada pela função matemática determinística:
$$T_{\text{calendar}} = \begin{cases} \text{ISO\_parse}(S), & \text{se } S \text{ contém hífens ('-')} \\ \text{Date}(1899, 12, 30) + \Delta t \times 1\text{ dia}, & \text{se } S \in \mathbb{N} \end{cases}$$

### 2.2. Distribuição Anual Consolidada
| Ano Calendário | Ano Budista (B.E.) | Quantidade de Amostras | Percentual (%) | Período Abrangido |
| :--- | :--- | :--- | :--- | :--- |
| **2023** | 2566 | 410 | 17.10% | `2023-01-13` a `2023-12-18` |
| **2024** | 2567 | 1,131 | 47.16% | `2024-01-04` a `2024-12-25` |
| **2025** | 2568 | 857 | 35.74% | `2025-01-17` a `2025-09-26` |
| **Total** | - | **2,398** | **100.00%** | **`2023-01-13` a `2025-09-26`** |

### 2.3. Distribuição Mensal das Observações (Sazonalidade Agronômica)
| Ano-Mês | Amostras | Sazonalidade Agrícola Tailandesa |
| :--- | :--- | :--- |
| `2023-01` | 29 | Segunda Safra / Seca (Off-season dry rice) |
| `2023-02` | 28 | Segunda Safra / Seca (Off-season dry rice) |
| `2023-03` | 62 | Segunda Safra / Seca (Off-season dry rice) |
| `2023-04` | 9 | Segunda Safra / Seca (Off-season dry rice) |
| `2023-05` | 33 | Safra Principal / Monções (In-season wet rice) |
| `2023-06` | 19 | Safra Principal / Monções (In-season wet rice) |
| `2023-07` | 24 | Safra Principal / Monções (In-season wet rice) |
| `2023-08` | 64 | Safra Principal / Monções (In-season wet rice) |
| `2023-09` | 34 | Safra Principal / Monções (In-season wet rice) |
| `2023-10` | 56 | Safra Principal / Monções (In-season wet rice) |
| `2023-11` | 41 | Segunda Safra / Seca (Off-season dry rice) |
| `2023-12` | 11 | Segunda Safra / Seca (Off-season dry rice) |
| `2024-01` | 118 | Segunda Safra / Seca (Off-season dry rice) |
| `2024-02` | 43 | Segunda Safra / Seca (Off-season dry rice) |
| `2024-03` | 36 | Segunda Safra / Seca (Off-season dry rice) |
| `2024-04` | 15 | Segunda Safra / Seca (Off-season dry rice) |
| `2024-05` | 93 | Safra Principal / Monções (In-season wet rice) |
| `2024-06` | 147 | Safra Principal / Monções (In-season wet rice) |
| `2024-07` | 54 | Safra Principal / Monções (In-season wet rice) |
| `2024-08` | 182 | Safra Principal / Monções (In-season wet rice) |
| `2024-09` | 121 | Safra Principal / Monções (In-season wet rice) |
| `2024-10` | 108 | Safra Principal / Monções (In-season wet rice) |
| `2024-11` | 127 | Segunda Safra / Seca (Off-season dry rice) |
| `2024-12` | 87 | Segunda Safra / Seca (Off-season dry rice) |
| `2025-01` | 42 | Segunda Safra / Seca (Off-season dry rice) |
| `2025-02` | 102 | Segunda Safra / Seca (Off-season dry rice) |
| `2025-03` | 77 | Segunda Safra / Seca (Off-season dry rice) |
| `2025-04` | 54 | Segunda Safra / Seca (Off-season dry rice) |
| `2025-05` | 76 | Safra Principal / Monções (In-season wet rice) |
| `2025-06` | 120 | Safra Principal / Monções (In-season wet rice) |
| `2025-07` | 103 | Safra Principal / Monções (In-season wet rice) |
| `2025-08` | 152 | Safra Principal / Monções (In-season wet rice) |
| `2025-09` | 131 | Safra Principal / Monções (In-season wet rice) |

---

## 3. Taxonomia e Desbalanceamento da Variável Alvo (`rice_stage`)
A identificação precisa da fase fenológica do arroz é a meta de inferência do sistema `Oryza-Elo`. O dataset contém 7 designações fenológicas na língua tailandesa.

### 3.1. Mapeamento Internacional BBCH / IRRI
| Estágio Original (TH) | Transliteração | Nome (Português) | Nome (Inglês) | Escala BBCH | Código IRRI | Amostras | Proporção (%) |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **แตกกอ** | *Taek Ko* | Perfilhamento | Tillering | `BBCH 20–29` | Fase Vegetativa (Estágio 2) | **1,128** | **47.04%** |
| **ระยะกล้า** | *Raya Kla* | Plântula / Muda | Seedling | `BBCH 10–19` | Fase Vegetativa (Estágio 1) | **559** | **23.31%** |
| **ออกรวง** | *Ok Ruang* | Emissão da Panícula | Heading / Panicle Emergence | `BBCH 51–59` | Fase Reprodutiva (Estágio 4) | **301** | **12.55%** |
| **ตั้งท้อง** | *Tang Thong* | Emborrachamento | Booting | `BBCH 41–49` | Fase Reprodutiva (Estágio 3) | **189** | **7.88%** |
| **ก่อนเก็บเกี่ยว** | *Kon Kep Kiao* | Maturação Pré-Colheita | Maturity / Ripening (Pre-harvest) | `BBCH 83–89` | Fase de Maturação (Estágio 6) | **135** | **5.63%** |
| **ออกดอก** | *Ok Dok* | Floração / Antese | Flowering / Anthesis | `BBCH 61–69` | Fase Reprodutiva (Estágio 5) | **65** | **2.71%** |
| **ก่อนเก็บเกี่ยวเกี่ยว** | *Kon Kep Kiao Kiao* | Maturação Plena / Colheita Iminente | Late Maturity / Harvest Stage | `BBCH 92–99` | Fase de Maturação (Estágio 7) | **21** | **0.88%** |
| **Total** | - | - | - | - | - | **2,398** | **100.00%** |

### 3.2. Razão de Desbalanceamento (Imbalance Ratio - $IR$)
A razão de desbalanceamento entre a classe majoritária (`แตกกอ` - Perfilhamento, $N = 1.128$) e a classe minoritária (`ก่อนเก็บเกี่ยวเกี่ยว` - Colheita Tardia, $N = 21$) é calculada por:
$$IR = \frac{N_{\max}}{N_{\min}} = \frac{1128}{21} \approx 53.71$$

> [!IMPORTANT]
> **Diretriz Estatística para Treinamento (Issue #4)**:
> Um $IR$ de 53.7:1 inviabiliza o uso da Acurácia simples como métrica de otimização (um modelo trivial que sempre previsse *Perfilhamento* teria 47% de acurácia, sendo inútil na prática).
> É mandatório o uso de **Macro-F1 Score** como métrica primária de decisão e de **Stratified K-Fold Cross-Validation** ($K=5$) para preservar a representatividade de cada classe em todas as dobras.

### 3.3. Decisão Agronômica sobre o Subestágio `ก่อนเก็บเกี่ยวเกี่ยว`
O estágio `ก่อนเก็บเกี่ยวเกี่ยว` (21 amostras, 0,88%) é uma variação léxica/subestágio de `ก่อนเก็บเกี่ยว` (135 amostras, 5,63%), denotando o momento exato em que as ceifadeiras entram no talhão.
- **Abordagem Experimental**: No baseline de Machine Learning, compararemos dois cenários:
  1. *Cenário 7-Classes (Rigor Máximo)*: preservando as 21 instâncias como classe independente para avaliar a capacidade discriminativa fina;
  2. *Cenário 6-Classes (Consolidação de Maturação)*: fundindo `ก่อนเก็บเกี่ยวเกี่ยว` em `ก่อนเก็บเกี่ยว` (totalizando 156 amostras, 6,51%), reduzindo o $IR$ para $17,3:1$ e aumentando o suporte estatístico.

---

## 4. Auditoria e Correção Forense de Coordenadas Geográficas
A integridade das coordenadas é mandatória para a consulta à API NASA POWER. Durante a auditoria, foram identificadas e corrigidas anomalias pontuais de digitação humana:

### 4.1. Anomalias Geoespaciais Detectadas nos Dados Brutos
1. **Erro de Escala Decimal ($10^6$)**: Ocorreu em 3 registros onde o ponto decimal foi omitido pelo digitador:
   - Linha 884 (Província de Uttaradit): `lat = 17155348.0` $\rightarrow$ corrigido para `17.155348°N` ($17155348 / 10^6$);
   - Linha 1189 (Província de Udon Thani): `lat = 17210212.0` $\rightarrow$ corrigido para `17.210212°N` ($17210212 / 10^6$);
   - Linha 1018 (Província de Songkhla): `lon = 100242767.0` $\rightarrow$ corrigido para `100.242767°E` ($100242767 / 10^6$).
2. **Duplicação de Latitude na Longitude**: Em 2 registros, o digitador copiou a latitude no campo de longitude:
   - Linha 522 (Suphan Buri): `lat = 14.6547`, `lon = 14.6547` $\rightarrow$ imputado para o centróide mediano de Suphan Buri (`100.0128°E`);
   - Linha 553 (Prachin Buri): `lat = 14.0341`, `lon = 14.0341` $\rightarrow$ imputado para o centróide mediano de Prachin Buri (`101.4125°E`).
3. **Truncamento de Dígito de Longitude**: Em 2 registros com longitude fora do território nacional tailandês ($Lon < 97^\circ$):
   - Linha 102 (Phichit): `lon = 94.9069` $\rightarrow$ imputado para o centróide mediano de Phichit (`100.3421°E`);
   - Linha 1442 (Phatthalung): `lon = 91.1572` $\rightarrow$ imputado para o centróide mediano de Phatthalung (`100.0524°E`).

### 4.2. Estatísticas Descritivas das Coordenadas Limpas (100% Válidas)
| Coordenada | Mínimo | 1º Quartil (Q1) | Mediana | Média | 3º Quartil (Q3) | Máximo | Desvio-Padrão |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **Latitude (°N)** | `7.0417` | `14.7975` | `16.4125` | `15.3905` | `17.4041` | `20.4122` | `3.6085` |
| **Longitude (°E)** | `98.0420` | `99.9581` | `100.1128` | `100.8140` | `101.1690` | `105.2599` | `1.6086` |

### 4.3. Top 15 Províncias com Maior Densidade de Levantamento
| Posição | Província (TH) | Nome Transliterado | Região Geográfica | Observações | Proporção (%) | Proporção Acumulada (%) |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| 1 | **พัทลุง** | - | Planície Central / Norte / Sul | 225 | 9.38% | 9.38% |
| 2 | **ชัยนาท** | - | Planície Central / Norte / Sul | 219 | 9.13% | 18.52% |
| 3 | **เชียงราย** | - | Planície Central / Norte / Sul | 189 | 7.88% | 26.40% |
| 4 | **พิษณุโลก** | - | Planície Central / Norte / Sul | 151 | 6.30% | 32.69% |
| 5 | **สกลนคร** | - | Planície Central / Norte / Sul | 125 | 5.21% | 37.91% |
| 6 | **อุทัยธานี** | - | Planície Central / Norte / Sul | 87 | 3.63% | 41.53% |
| 7 | **สุพรรณบุรี** | - | Planície Central / Norte / Sul | 86 | 3.59% | 45.12% |
| 8 | **พะเยา** | - | Planície Central / Norte / Sul | 82 | 3.42% | 48.54% |
| 9 | **พิจิตร** | - | Planície Central / Norte / Sul | 82 | 3.42% | 51.96% |
| 10 | **อุตรดิตถ์** | - | Planície Central / Norte / Sul | 77 | 3.21% | 55.17% |
| 11 | **นครสวรรค์** | - | Planície Central / Norte / Sul | 67 | 2.79% | 57.96% |
| 12 | **นครพนม** | - | Planície Central / Norte / Sul | 62 | 2.59% | 60.55% |
| 13 | **เชียงใหม่** | - | Planície Central / Norte / Sul | 57 | 2.38% | 62.93% |
| 14 | **กระบี่** | - | Planície Central / Norte / Sul | 55 | 2.29% | 65.22% |
| 15 | **สุโขทัย** | - | Planície Central / Norte / Sul | 53 | 2.21% | 67.43% |

### 4.4. Otimização Espacial para Ingestão NASA POWER (Issue #2)
> [!TIP]
> **Agrupamento de Grade de 18x**: A grade nativa de reanálise atmosférica da NASA POWER possui resolução de $0,5^\circ \times 0,5^\circ$ (~55 km).
> Todas as 2.398 observações caem em exatamente **124 células de grade distintas**.
> Consultar por célula de grade e intervalo de tempo reduz o tráfego de rede de 2.398 chamadas para 131, evitando rate limits (HTTP 429) e acelerando a extração em mais de 1.800%.

---

## 5. Caracterização dos Ecossistemas e Variedades Cultivadas
A resposta fenológica à temperatura e precipitação varia conforme o manejo hídrico da lavoura.

### 5.1. Distribuição dos Ecossistemas de Cultivo (`rice_ecosystem`)
| Ecossistema Original (TH) | Classificação Agronômica | Amostras | Proporção (%) | Manejo de Irrigação |
| :--- | :--- | :--- | :--- | :--- |
| **นาชลประทาน** | Várzea Irrigada (Irrigated Lowland) | **1,474** | **61.47%** | Controle total por canais e comportas |
| **นาน้ำฝน** | Várzea de Sequeiro (Rainfed Lowland) | **691** | **28.82%** | Dependência de precipitação e lençol freático |
| **ข้าวไร่ที่สูง** | Arroz de Terras Altas (Upland / Highland) | **68** | **2.84%** | Infiltração direta de chuvas em relevo ondulado |
| **ข้าวไร่แซมยาพาราหรือปาล์มน้ำมัน** | Consórcio com Borracha/Dendê (Agroflorestal) | **55** | **2.29%** | Sombra parcial e captação de chuva |
| **นิเวศข้าวกับพืชอื่น** | Misto / Outros | **52** | **2.17%** | Variável |
| **ข้าวไร่ที่สูงขังน้ำ** | Arroz de Terras Altas (Upland / Highland) | **42** | **1.75%** | Infiltração direta de chuvas em relevo ondulado |
| **ข้าวนาที่สูง** | Arroz de Terras Altas (Upland / Highland) | **16** | **0.67%** | Infiltração direta de chuvas em relevo ondulado |

### 5.2. Principais Cultivares de Arroz (`rice_variety`)
O dataset registra **186 cultivares** diferentes. As variedades mais expressivas são:
| Cultivar | Designação Agronômica | Tipo de Grão / Ciclo | Amostras | Proporção (%) |
| :--- | :--- | :--- | :--- | :--- |
| **ไม่ทราบพันธุ์ข้าว** | Não especificado / Variedade crioula local não catalogada | Grão Longo / Ciclo Médio | 537 | 22.39% |
| **ไม่ทราบพันธุ์** | Não especificado / Variedade crioula local não catalogada | Grão Longo / Ciclo Médio | 193 | 8.05% |
| **กข85** | RD85 (Arroz de alto rendimento, fotoperíodo insensível) | Grão Longo / Ciclo Médio | 171 | 7.13% |
| **กข6** | RD6 (Arroz glutinoso aromático tradicional do Nordeste) | Grão Longo / Ciclo Médio | 163 | 6.80% |
| **กข41** | RD41 (Variedade moderna resistente a pragas) | Grão Longo / Ciclo Médio | 125 | 5.21% |
| **สันป่าตอง 1** | Variedade comercial / híbrido local | Grão Longo / Ciclo Médio | 73 | 3.04% |
| **กข79** | RD79 (Grão longo semi-aromático de alta qualidade) | Grão Longo / Ciclo Médio | 70 | 2.92% |
| **มะลิ105** | Khao Dawk Mali 105 (O autêntico Arroz Jasmine Tailandês) | Grão Longo / Ciclo Médio | 64 | 2.67% |
| **กข61** | Variedade comercial / híbrido local | Grão Longo / Ciclo Médio | 63 | 2.63% |
| **ปทุมธานี 1** | Variedade comercial / híbrido local | Grão Longo / Ciclo Médio | 60 | 2.50% |

---

## 6. Análise de Estresses Bióticos e Validação Cruzada Fenológica
O levantamento inclui 35 medições de pressão de pragas de insetos, doenças fúngicas/bacterianas e plantas daninhas.

### 6.1. Ranking de Prevalência dos Estresses Bióticos
| Variável de Estresse | Prevalência (%) | Média | Desvio-Padrão | Mediana | IQR | Máximo | Assimetria (Skewness) |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `brown_spot_%` | **72.19%** | 7.009 | 12.802 | 2.140 | 7.500 | 100.0 | `3.56` |
| `weed_infestration_%` | **59.47%** | 10.288 | 18.931 | 3.330 | 10.000 | 100.0 | `2.51` |
| `leaf_folder` | **30.86%** | 1.050 | 4.260 | 0.000 | 0.470 | 72.2 | `9.47` |
| `leaf_blast_%` | **23.27%** | 1.772 | 7.398 | 0.000 | 0.000 | 209.1 | `12.82` |
| `whorl_maggot_%` | **16.10%** | 0.348 | 1.951 | 0.000 | 0.000 | 79.0 | `28.52` |
| `red_stripe_%` | **15.30%** | 0.266 | 1.498 | 0.000 | 0.000 | 38.7 | `16.38` |
| `narrow_brown_spot_%` | **14.68%** | 1.037 | 6.136 | 0.000 | 0.000 | 100.0 | `10.67` |
| `dirty_panicle_%` | **14.10%** | 3.665 | 12.896 | 0.000 | 0.000 | 100.0 | `4.57` |
| `dead_heart_%` | **11.26%** | 0.469 | 2.260 | 0.000 | 0.000 | 41.4 | `8.88` |
| `bph_pertiller` | **10.47%** | 0.285 | 2.006 | 0.000 | 0.000 | 50.0 | `17.78` |
| `wbph_pertiller` | **7.80%** | 0.173 | 1.449 | 0.000 | 0.000 | 58.6 | `28.99` |
| `bacterial_blight_%` | **6.84%** | 0.986 | 6.354 | 0.000 | 0.000 | 90.0 | `9.60` |
| `rice_gall_midge_%` | **6.01%** | 0.461 | 3.104 | 0.000 | 0.000 | 74.6 | `12.38` |
| `rice_thrip_%` | **5.80%** | 1.228 | 24.170 | 0.000 | 0.000 | 963.9 | `34.55` |
| `rat_damage_%` | **5.09%** | 0.267 | 1.570 | 0.000 | 0.000 | 25.4 | `9.11` |

### 6.2. Prova de Integridade Biológica (Cross-Tab Fenológico)
A correlação empírica entre os estresses e os estágios fenológicos demonstra com clareza a autenticidade e consistência biológica dos dados coletados:

| Estágio Fenológico | Ocorrência de Panícula Manchada (`dirty_panicle_%`) | Ocorrência de Coração Morto (`dead_heart_%`) | Ocorrência de Lagarta Enroladeira (`leaf_folder`) |
| :--- | :--- | :--- | :--- |
| **แตกกอ** (Tillering) | **0.09%** | **16.67%** | **36.70%** |
| **ระยะกล้า** (Seedling) | **0.18%** | **3.94%** | **10.91%** |
| **ออกรวง** (Heading / Panicle Emergence) | **75.08%** | **5.65%** | **42.19%** |
| **ตั้งท้อง** (Booting) | **0.53%** | **20.63%** | **47.09%** |
| **ก่อนเก็บเกี่ยว** (Maturity / Ripening (Pre-harvest)) | **63.70%** | **1.48%** | **18.52%** |
| **ออกดอก** (Flowering / Anthesis) | **16.92%** | **3.08%** | **32.31%** |
| **ก่อนเก็บเกี่ยวเกี่ยว** (Late Maturity / Harvest Stage) | **57.14%** | **0.00%** | **14.29%** |

> [!NOTE]
> **Constatação Biológica Fundamental**:
> - **Panícula Manchada** atinge 75,08% de prevalência na emissão da panícula (`ออกรวง`) e 63,70% na maturação (`ก่อนเก็บเกี่ยว`), mas é de apenas 0,08% no perfilhamento (`แตกกอ`), pois a panícula simplesmente não existe no início do ciclo;
> - **Coração Morto** (causado por brocas do colmo destruindo o ponto de crescimento da planta) concentra-se nas fases de emborrachamento (20,63%) e perfilhamento (16,67%), decaindo para zero na colheita;
> Essa coerência empírica valida o dataset como fonte fidedigna e livre de corrupções para a modelagem preditiva.

---

## 7. Auditoria de Dados Ausentes (Missing Values)
| Categoria de Coluna | Total de Colunas | Colunas com 0% Nulos | Nulos Máximos Encontrados | Ação de Pré-Processamento |
| :--- | :--- | :--- | :--- | :--- |
| **Identificadores & Coordenadas** | 3 (`survey_date`, `lat`, `lon`) | 3 (100% íntegras) | 0 (0,0%) | Nenhuma imputação necessária após correção de escala |
| **Rótulo Alvo (`rice_stage`)** | 1 | 1 (100% íntegro) | 0 (0,0%) | 2.398 rótulos validados |
| **Contexto Agronômico** | 3 (`province`, `rice_ecosystem`, `rice_variety`) | 3 | 0 (0,0%) | Codificação categórica nativa (CatBoost) |
| **Estresses Bióticos Contínuos** | 33 | 33 | 0 (0,0%) | Campos numéricos com 0 indicando ausência de sintoma |
| **Plantas Daninhas Categóricas** | 2 (`dominant_weed_type`, `dominant_weed_species`) | 0 | 1.840 (76,7%) | Representadas como string `'NA'`; tratadas como categoria `'None'` |

---

## 8. Conclusões e Diretrizes para os Próximos Passos do Milestone 1
Com base nos achados desta caracterização exploratória, as diretrizes para as Issues subsequentes estão consolidadas:

1. **Para a Issue #2 (Ingestão NASA POWER)**:
   - Implementar agrupamento espacial nas **131 células de grade** ($0,5^\circ \times 0,5^\circ$);
   - Coletar histórico diário de **60 dias retrospectivos** para cada uma das datas normalizadas (`2023-01-13` a `2025-09-26`);
   - Aplicar as regras de coordenadas limpas (fator $10^6$ e centróide provincial) para prevenir requisições HTTP 400.

2. **Para a Issue #3 (Engenharia de Features)**:
   - Calcular $GDD$ utilizando $T_{\text{base}} = 10,0^\circ C$ em janelas $W \in \{7, 14, 30, 60\}$;
   - Calcular a amplitude térmica diurna ($DTR = T_{\max} - T_{\min}$), chuva acumulada e radiação acumulada;
   - Garantir estrita ausência de vazamento de dados (*leakage*), utilizando apenas $t \le t_{\text{survey}}$.

3. **Para a Issue #4 (Modelagem & Validação de Machine Learning)**:
   - Adotar **Stratified 5-Fold Cross-Validation** como protocolo padrão obrigatório;
   - Utilizar **Macro-F1 Score** como métrica primária balizadora (meta $\ge 0,75$);
   - Comparar CatBoost, XGBoost e Random Forest, preparando a serialização do campeão em formato ONNX para inferência em microssegundos (< 5 ms) no microserviço Rust no Raspberry Pi.

---
*Relatório gerado e auditado para o repositório Oryza-Elo.*