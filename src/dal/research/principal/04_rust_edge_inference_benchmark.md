# Relatório Técnico 04: Benchmark de Inferência de Borda em Rust (ONNX Runtime)

## 1. Sumário Executivo

Este documento formaliza a avaliação experimental da latência de inferência do modelo supervisionado de classificação fenológica do arroz (*Oryza sativa L.*) empacotado em ONNX e executado nativamente em Rust através da crate `ort` (ONNX Runtime bindings 2.0).

Em 2026-10-06 a latência foi medida com o mesmo protocolo em dois ambientes: um Raspberry Pi 5 (hardware de borda, aarch64) e o computador do autor (x86_64). Na Pi 5, a latência média foi de **56,8 µs por predição** (média das 5 rodadas), cerca de **3.500 vezes abaixo** do teto de 200 ms definido na hipótese do pré-projeto do TCC.

## 2. Protocolo de Medição

- **Endpoint**: `GET /api/v1/benchmarks/latency?iterations=1000` do próprio engine (`src/presentation/api/handlers/benchmark.rs`).
- **Por rodada**: 5 inferências de aquecimento (*warmup*), seguidas de 1.000 inferências unitárias cronometradas com `std::time::Instant` dentro do processo. A rede não entra na medição.
- **Entrada**: amostra fixa de 44 features (`sample_features()`), a mesma em todas as rodadas.
- **Rodadas**: 5 por ambiente, com 2 s de intervalo.
- **Binário**: compilação `--release` a partir de `feature/main`. Na Pi, o modelo ONNX tem o mesmo SHA-256 do repositório (`bedc3192...b27cf53`).

## 3. Ambientes

| Item | Raspberry Pi 5 (borda) | Computador do autor |
| :--- | :--- | :--- |
| Modelo | Raspberry Pi 5 Model B Rev 1.0 | Notebook com Intel Core i7-13700HX |
| CPU | ARM Cortex-A76, 4 núcleos, até 2,4 GHz | Intel Core i7-13700HX |
| Arquitetura | aarch64 | x86_64 |
| RAM | 8 GB | 16 GB |
| Sistema | Debian 13 (trixie), kernel 6.18.50+rpt-rpi-2712 | Debian 13, kernel 6.12.95 |
| Governador de CPU | `ondemand` | `powersave` |
| Execução | serviço systemd (`install.sh`) | processo temporário |
| Temperatura | 48,8 °C antes e 48,3 °C depois | não registrada |
| Throttling | nenhum (`get_throttled=0x0` antes e depois) | não registrado |

## 4. Resultados por Rodada (µs)

### Raspberry Pi 5 (aarch64)

| Rodada | Média | p50 | p95 | p99 | Mínimo | Máximo |
| :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| 1 | 56,39 | 56,76 | 59,04 | 69,04 | 48,59 | 123,00 |
| 2 | 61,72 | 64,96 | 67,50 | 80,04 | 49,04 | 115,37 |
| 3 | 59,74 | 57,32 | 66,30 | 76,69 | 54,43 | 117,72 |
| 4 | 64,60 | 64,13 | 67,30 | 79,52 | 60,87 | 95,47 |
| 5 | 41,51 | 41,21 | 43,26 | 53,26 | 38,80 | 62,48 |
| **Média das rodadas** | **56,79** | | | | | |

### Computador do autor (x86_64)

| Rodada | Média | p50 | p95 | p99 | Mínimo | Máximo |
| :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| 1 | 13,84 | 10,04 | 45,61 | 55,47 | 9,81 | 64,30 |
| 2 | 20,78 | 9,76 | 67,01 | 86,77 | 9,48 | 128,80 |
| 3 | 15,09 | 10,41 | 55,55 | 62,75 | 10,17 | 85,83 |
| 4 | 14,69 | 9,99 | 54,94 | 62,51 | 9,78 | 87,61 |
| 5 | 21,35 | 17,20 | 67,01 | 86,95 | 10,16 | 114,00 |
| **Média das rodadas** | **17,15** | | | | | |

A variação entre rodadas na Pi (41,5 a 64,6 µs) é compatível com o governador `ondemand`, que ajusta a frequência da CPU conforme a carga. No x86_64, a mediana fica perto de 10 µs e as caudas (p95 e p99) são maiores, efeito típico do governador `powersave` e dos núcleos híbridos do processador.

## 5. Comparação com os Tetos

| Teto | Origem | Pior valor na Pi 5 | Folga |
| :--- | :--- | :--- | :--- |
| 200 ms | hipótese do pré-projeto do TCC | p99 de 80,04 µs (rodada 2) | cerca de 2.500x |
| 5 ms | meta de engenharia do engine | p99 de 80,04 µs (rodada 2) | cerca de 62x |

Mesmo o maior valor isolado medido na Pi (123 µs) fica cerca de 1.600 vezes abaixo de 200 ms.

## 6. Arquitetura do Tensor de Borda

O modelo consome estritamente um tensor unidimensional com formato `[1, 44]` de floats de 32 bits (`f32`), ordenado rigorosamente conforme `features_order` definido em `model_metadata.json`:

1. **32 Features Climáticas Retrospectivas**: Janelas móveis de 7, 14, 30 e 60 dias para GDD (base 10°C), Precipitação acumulada, Chuva diária máxima, Dias consecutivos secos (CDD), Média da amplitude térmica diurna (DTR), Desvio padrão amostral da amplitude térmica (DTR std), Radiação solar global acumulada e Umidade relativa média.
2. **9 Features Biofísicas Derivadas**: Mês de observação, Dia juliano do ano (DOY), Fotoperíodo astronômico analítico (duração do dia em horas com clamp de declinação solar), Quociente Fototérmico (PTQ em 30 e 60 dias), Proxy de Déficit de Pressão de Vapor (VPD em 14 e 30 dias), Latitude e Longitude.
3. **3 Encodings Categóricos Numéricos**: Código de agro-ecossistema do arroz (0..6), código de cultivar/variedade (0..185) e código de província administrativa tailandesa (0..57).

## 7. Memória e Execução Contínua

- **Sessão Singleton Residente**: O arquivo ONNX (3,2 MB) é carregado na memória uma única vez no boot da aplicação, evitando leitura de disco a cada predição.
- **Memória residente medida na Pi 5**: 68,4 MB (RSS) para o processo inteiro do engine (servidor HTTP, SQLite, agendador e ONNX Runtime), em repouso após o benchmark. O valor anterior deste relatório (cerca de 18 MB) não tinha medição registrada.
- **Sincronização Thread-Safe**: A sessão é envelopada em um `std::sync::Mutex<Session>` para acesso concorrente seguro ao runtime C do ONNX.
- **Execução contínua**: o serviço ficou ativo na Pi durante a medição; operação contínua por longos períodos e consumo energético ainda não foram medidos.

## 8. Medição Anterior (x86_64)

Uma medição anterior no mesmo computador do autor, com outro roteiro (50 iterações de aquecimento), registrou média de 20,889 µs, p50 de 20,599 µs, p95 de 22,840 µs e p99 de 25,144 µs. Os números da seção 4 substituem esta medição por usarem o mesmo protocolo da Pi.

## 9. Conclusão

No Raspberry Pi 5, hardware de borda de baixo custo, a inferência do modelo ficou em 56,8 µs por predição em média, com p99 máximo de 80 µs e sem throttling térmico. Isso coloca o modelo cerca de 3.500 vezes abaixo do teto de 200 ms da hipótese do pré-projeto. No computador do autor, a média foi de 17,2 µs, cerca de 3,3 vezes mais rápida que na Pi.

---
*Revisado em 2026-10-05: inclusão do ambiente de medição, correção da origem do teto de 200 ms (hipótese do pré-projeto, não exigência do orientador) e remoção de afirmações sobre execução em Raspberry Pi e sobre redução de latência frente ao Python, que não foram medidas.*

*Atualizado em 2026-10-06 (Issue #19): medição no Raspberry Pi 5 e no computador do autor com o mesmo protocolo, ficha do hardware, memória residente medida na Pi.*
