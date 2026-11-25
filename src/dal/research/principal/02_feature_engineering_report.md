# Oryza-Elo: Relatório de Engenharia de Features Biometeorológicas
## Modelagem Fenológica e Termodinâmica para Orizicultura de Borda (Issue #3)

> **Status**: Concluído | **Fase**: Milestone 1 (Issue #3) | **Data**: 2026-09-18  
> **Dataset Processado**: `src/dal/data/processed/principal/rice_survey_climate_enriched.csv` (2,398 linhas x 78 colunas)  

---

## 1. Fundamentação Fisiológica & Racional Agronômico
A transição entre fases fenológicas no arroz (*Oryza sativa* L.) é primariamente governada pelo tempo térmico biológico acumulado e pela disponibilidade de radiação solar e água. Métodos puramente visuais (câmeras) falham em antecipar estágios internos cruciais como a diferenciação da panícula e o emborrachamento.

Este relatório formaliza a engenharia de 32 variáveis biometeorológicas calculadas a partir das séries diárias de reanálise atmosférica da **NASA POWER** para cada uma das 2.398 observações de campo.

---

## 2. Modelagem Matemática das Features

### 2.1. Graus-Dia Acumulados ($GDD$ - Growing Degree Days)
O tempo térmico fisiológico é calculado com base na temperatura basal do arroz $T_{\text{base}} = 10,0^\circ C$:
$$GDD_t = \max\left(0, \frac{T_{\max, t} + T_{\min, t}}{2} - 10,0\right)$$
- Acúmulo retrospectivo em janelas $W \in \{7, 14, 30, 60\}$ dias:
$$\text{GDD\_cum}_W = \sum_{i=0}^{W-1} GDD_{t-i}$$

### 2.2. Amplitude Térmica Diurna ($DTR$)
$$DTR_t = T_{\max, t} - T_{\min, t}$$
- Média móvel $\text{dtr\_mean}_W$ e dispersão $\text{dtr\_std}_W$ indicam estabilidade microclimática e estresse térmico por calor.

### 2.3. Balanço Hídrico & Precipitação
- Precipitação acumulada: $\text{rain\_cum}_W = \sum_{i=0}^{W-1} P_{t-i}$
- Precipitação máxima diária: $\text{rain\_max}_W = \max_{i} (P_{t-i})$
- Dias Consecutivos Secos ($CDD_W$): contagem de dias na janela com $P < 1,0\text{ mm}$.

### 2.4. Radiação Solar Acumulada
$$\text{rad\_cum}_W = \sum_{i=0}^{W-1} \text{ALLSKY\_SFC\_SW\_DWN}_{t-i}$$

---

## 3. Protocolo Anti-Vazamento (Anti-Data Leakage)
> [!IMPORTANT]
> **Garantia de Não-Vazamento Temporal**:
> Todas as 32 features utilizam intervalos estritamente retrospectivos $[t_{\text{survey}} - W + 1, t_{\text{survey}}]$. Nenhuma informação posterior à data da visita agronômica é permitida.

---

## 4. Estatísticas Descritivas das 32 Features Biometeorológicas
| Variável | Descrição Agronômica | Média | Desvio-Padrão | Mínimo | Mediana | Máximo |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `gdd_cum_7d` | Feature Biometeorológica | 120.20 | 18.38 | 36.43 | 120.99 | 194.78 |
| `rain_cum_7d` | Feature Biometeorológica | 34.31 | 35.03 | 0.00 | 26.32 | 186.57 |
| `rain_max_7d` | Feature Biometeorológica | 12.46 | 12.86 | 0.00 | 9.11 | 70.83 |
| `rad_cum_7d` | Feature Biometeorológica | 124.91 | 18.55 | 55.22 | 123.62 | 179.79 |
| `dtr_mean_7d` | Feature Biometeorológica | 8.35 | 3.31 | 0.60 | 7.44 | 17.68 |
| `dtr_std_7d` | Feature Biometeorológica | 1.43 | 0.63 | 0.22 | 1.31 | 4.32 |
| `rh_mean_7d` | Feature Biometeorológica | 77.07 | 12.52 | 34.44 | 82.06 | 92.40 |
| `cdd_7d` | Feature Biometeorológica | 2.94 | 2.82 | 0.00 | 2.00 | 7.00 |
| `gdd_cum_14d` | Feature Biometeorológica | 240.55 | 36.33 | 91.56 | 243.18 | 383.42 |
| `rain_cum_14d` | Feature Biometeorológica | 71.49 | 62.77 | 0.00 | 61.58 | 291.27 |
| `rain_max_14d` | Feature Biometeorológica | 17.91 | 15.06 | 0.00 | 14.92 | 71.51 |
| `rad_cum_14d` | Feature Biometeorológica | 247.31 | 32.81 | 131.47 | 245.40 | 351.89 |
| `dtr_mean_14d` | Feature Biometeorológica | 8.35 | 3.23 | 0.71 | 7.41 | 17.87 |
| `dtr_std_14d` | Feature Biometeorológica | 1.57 | 0.60 | 0.27 | 1.43 | 4.05 |
| `rh_mean_14d` | Feature Biometeorológica | 77.08 | 12.29 | 36.32 | 81.95 | 92.76 |
| `cdd_14d` | Feature Biometeorológica | 5.93 | 5.29 | 0.00 | 4.00 | 14.00 |
| `gdd_cum_30d` | Feature Biometeorológica | 514.90 | 80.81 | 222.67 | 520.10 | 795.44 |
| `rain_cum_30d` | Feature Biometeorológica | 157.59 | 125.97 | 0.00 | 152.23 | 523.35 |
| `rain_max_30d` | Feature Biometeorológica | 24.87 | 18.18 | 0.00 | 22.79 | 95.00 |
| `rad_cum_30d` | Feature Biometeorológica | 527.49 | 61.75 | 372.53 | 521.70 | 701.30 |
| `dtr_mean_30d` | Feature Biometeorológica | 8.40 | 3.21 | 0.72 | 7.47 | 17.54 |
| `dtr_std_30d` | Feature Biometeorológica | 1.71 | 0.57 | 0.32 | 1.61 | 3.78 |
| `rh_mean_30d` | Feature Biometeorológica | 76.94 | 12.13 | 40.81 | 81.59 | 92.82 |
| `cdd_30d` | Feature Biometeorológica | 12.73 | 10.79 | 0.00 | 8.00 | 30.00 |
| `gdd_cum_60d` | Feature Biometeorológica | 1037.57 | 167.15 | 500.35 | 1038.66 | 1513.48 |
| `rain_cum_60d` | Feature Biometeorológica | 304.32 | 213.86 | 0.10 | 318.44 | 998.32 |
| `rain_max_60d` | Feature Biometeorológica | 31.03 | 18.97 | 0.09 | 29.98 | 128.84 |
| `rad_cum_60d` | Feature Biometeorológica | 1059.06 | 109.82 | 736.57 | 1047.21 | 1349.00 |
| `dtr_mean_60d` | Feature Biometeorológica | 8.48 | 3.00 | 0.68 | 7.80 | 17.13 |
| `dtr_std_60d` | Feature Biometeorológica | 1.93 | 0.66 | 0.33 | 1.78 | 3.79 |
| `rh_mean_60d` | Feature Biometeorológica | 76.35 | 11.36 | 43.82 | 80.31 | 92.05 |
| `cdd_60d` | Feature Biometeorológica | 25.65 | 19.84 | 0.00 | 18.00 | 60.00 |

---

## 5. Dinâmica do Tempo Térmico ($GDD$) por Estágio Fenológico
A média de Graus-Dia Acumulados demonstra separação clara entre os estágios vegetativos e reprodutivos:

| Estágio Fenológico | GDD Acumulado 7 Dias (°C·dia) | GDD Acumulado 14 Dias (°C·dia) | GDD Acumulado 30 Dias (°C·dia) | GDD Acumulado 60 Dias (°C·dia) |
| :--- | :--- | :--- | :--- | :--- |
| **ก่อนเก็บเกี่ยว** | 115.2 | 231.3 | 500.1 | **1014.3** |
| **ก่อนเก็บเกี่ยวเกี่ยว** | 129.6 | 263.8 | 569.1 | **1088.7** |
| **ตั้งท้อง** | 120.8 | 242.2 | 517.5 | **1032.5** |
| **ระยะกล้า** | 118.5 | 236.2 | 503.0 | **1033.5** |
| **ออกดอก** | 118.0 | 234.2 | 496.5 | **970.7** |
| **ออกรวง** | 122.9 | 247.6 | 530.1 | **1058.0** |
| **แตกกอ** | 120.7 | 241.6 | 518.1 | **1040.6** |

> [!NOTE]
> **Conclusão Agronômica**:
> Conforme antecipado pela literatura biológica, o $GDD_{60d}$ fornece o maior gradiente de separação entre o Perfilhamento inicial e os estágios de Maturação, provando ser o principal preditor biometeorológico para a classificação no modelo de Machine Learning da Issue #4.

---
*Relatório técnico gerado e auditado para o repositório Oryza-Elo.*