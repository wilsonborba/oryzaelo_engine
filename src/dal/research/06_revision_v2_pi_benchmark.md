# Relatório Técnico 06: Benchmark do Candidato v2 de Seis Classes (Raspberry Pi 5 e MSI)

Medição do modelo exato `rice_stage_six.onnx` com o harness isolado `revision_v2/pi_benchmark/harness/`. O relatório 04 permanece como evidência histórica de outro modelo e não foi alterado.

## Artefatos

- Modelo: `5d4d7e72190d5419a01e1aa3e2328e164335f36f7272a2ed5d979687bfac9f49`
- Entrada `[1, 44]` float32: `2bc8edcb1514fffdbbf580a683f4fea4927ac52a637ce0feb267d952e7630338`
- Metadados: `d6e01952aa182c8bb9b95face916513296bdb7edf3efb30a139dc79a91ef3c10`
- Referência (numeric_six.cbm): `345a39576c96d6cfcd2eaaf5ebaf400ae0c348af6165a7d8476d5c3f2ecc78d6`

## Protocolo

- Cada rodada é um processo independente: uma sessão ONNX Runtime em CPU, nível de otimização 3, 1 thread intra-op, 1 thread inter-op, execução sequencial.
- Antes da medição, a saída é conferida: 6 probabilidades finitas, soma 1, classe prevista igual à referência e diferença absoluta máxima < 1e-5.
- 50 predições de aquecimento e 1.000 predições cronometradas com relógio monotônico dentro do processo. A predição cronometrada inclui a montagem do tensor, a execução da sessão e a extração do rótulo e das probabilidades.
- Carregamento do modelo, leitura de arquivos, inicialização do processo, compilação e transporte por SSH ficam fora da medição.
- Percentis por posto mais próximo: índice ceil(p × n) − 1 no vetor ordenado iniciado em zero.
- Memória: pico de RSS (VmHWM em KiB) lido depois do laço cronometrado e convertido para MB decimais (KiB × 1.024 / 10^6).
- Critério: p99 < 200 ms em todas as rodadas. Os serviços em produção não foram parados.

## Raspberry Pi 5 Model B (rasp5, aarch64)

ONNX Runtime: `ORT Build Info: git-branch=rel-1.28.0, git-commit-id=da9b5e3, fp8-kv-cache=1, build type=Release`

| Rodada | Média (µs) | Mín. (µs) | Máx. (µs) | p50 (µs) | p95 (µs) | p99 (µs) | p99 (ms) | Pico RSS (MB) | Critério |
| :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| 1 | 46.03 | 44.33 | 59.48 | 45.94 | 46.76 | 52.20 | 0.0522 | 66.1 | atendido |
| 2 | 45.58 | 43.72 | 58.22 | 45.48 | 46.50 | 49.63 | 0.0496 | 66.1 | atendido |
| 3 | 46.70 | 45.00 | 76.94 | 46.57 | 47.45 | 51.96 | 0.0520 | 66.1 | atendido |
| 4 | 46.25 | 44.80 | 66.13 | 46.17 | 46.89 | 51.63 | 0.0516 | 66.1 | atendido |
| 5 | 66.00 | 63.96 | 114.33 | 65.78 | 66.94 | 74.19 | 0.0742 | 66.1 | atendido |

Média das médias: 50.11 µs. Pior p99: 74.19 µs. Critério em todas as rodadas: atendido. Correção: classe 0 (ก่อนเก็บเกี่ยว) em todas as rodadas; maior diferença de probabilidade 4.34e-08.

Ambiente registrado em `revision_v2/pi_benchmark/rasp5/environment.txt`. Tempos brutos em `revision_v2/pi_benchmark/rasp5/round_*_durations.csv`.

## Notebook MSI (Intel Core i7-13700HX, x86_64)

ONNX Runtime: `ORT Build Info: git-branch=rel-1.28.0, git-commit-id=da9b5e3, fp8-kv-cache=1, build type=Release`

| Rodada | Média (µs) | Mín. (µs) | Máx. (µs) | p50 (µs) | p95 (µs) | p99 (µs) | p99 (ms) | Pico RSS (MB) | Critério |
| :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| 1 | 8.54 | 8.09 | 32.41 | 8.41 | 8.81 | 11.92 | 0.0119 | 64.4 | atendido |
| 2 | 8.72 | 8.29 | 33.51 | 8.57 | 9.10 | 12.18 | 0.0122 | 65.1 | atendido |
| 3 | 8.60 | 8.34 | 18.38 | 8.54 | 8.96 | 9.83 | 0.0098 | 64.4 | atendido |
| 4 | 8.57 | 8.04 | 43.00 | 8.42 | 8.89 | 12.03 | 0.0120 | 64.6 | atendido |
| 5 | 8.69 | 8.41 | 13.84 | 8.60 | 9.09 | 9.92 | 0.0099 | 64.7 | atendido |

Média das médias: 8.63 µs. Pior p99: 12.18 µs. Critério em todas as rodadas: atendido. Correção: classe 0 (ก่อนเก็บเกี่ยว) em todas as rodadas; maior diferença de probabilidade 4.34e-08.

Ambiente registrado em `revision_v2/pi_benchmark/msi/environment.txt`. Tempos brutos em `revision_v2/pi_benchmark/msi/round_*_durations.csv`.

## Limites

- O ensaio mede apenas o tempo de inferência de um vetor fixo. Não mede acurácia, consumo de energia nem confiabilidade em campo.
- A acurácia deste candidato numérico é a registrada em `catboost_numeric_metrics.json` (Macro-F1 agrupado 0,6038), inferior ao critério de 0,75.
- Os dois hosts usam o mesmo código, modelo, entrada, protocolo e ONNX Runtime 1.28.0, mas compiladores Rust diferentes (rustc 1.99.0 no Pi, rustc 1.91.1 no MSI; registrados em `toolchain.txt`).
- Pi: governador `ondemand`. Os registros antes e depois das rodadas 1 a 4 mostram 2.400 MHz; o registro depois da rodada 5 mostra 1.600 MHz. A rodada 5 teve média maior (cerca de 66 µs contra cerca de 46 µs). Temperatura abaixo de 50 °C e `throttled=0x0` em todos os registros. O resultado da rodada 5 foi mantido, sem repetição.
- MSI: governador `powersave`, com frequência instantânea variável entre os registros.
