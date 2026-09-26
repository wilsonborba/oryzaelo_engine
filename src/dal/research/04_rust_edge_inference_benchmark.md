# Relatório Técnico 04: Benchmark de Inferência de Borda em Rust (ONNX Runtime)

## 1. Sumário Executivo

Este documento formaliza a avaliação experimental da latência de inferência do modelo supervisionado de classificação fenológica do arroz (*Oryza sativa L.*) empacotado em ONNX e executado nativamente em Rust através da crate `ort` (ONNX Runtime bindings 2.0).

O benchmark foi executado em ambiente CPU com 1.000 iterações unitárias após 50 iterações de aquecimento (*warmup*).

| Métrica | Valor Obtido (Rust + ONNX) | Meta Borda Rust | Teto Orientador (USP/ESALQ) | Status |
| :--- | :--- | :--- | :--- | :--- |
| **Latência Média** | **21.826 µs (0.0218 ms)** | < 5,0 ms | < 200,0 ms | **Aprovado (229.1x mais rápido)** |
| **Mediana (p50)** | **21.080 µs (0.0211 ms)** | < 5,0 ms | < 200,0 ms | **Aprovado** |
| **Percentil 95 (p95)** | **24.634 µs (0.0246 ms)** | < 5,0 ms | < 200,0 ms | **Aprovado** |
| **Percentil 99 (p99)** | **30.506 µs (0.0305 ms)** | < 5,0 ms | < 200,0 ms | **Aprovado** |
| **Mínimo Absoluto** | **20.571 µs** | - | - | - |
| **Máximo Absoluto** | **55.439 µs** | - | - | - |

## 2. Arquitetura do Tensor de Borda

O modelo consome estritamente um tensor unidimensional com formato `[1, 44]` de floats de 32 bits (`f32`), ordenado rigorosamente conforme `features_order` definido em `model_metadata.json`:

1. **32 Features Climáticas Retrospectivas**: Janelas móveis de 7, 14, 30 e 60 dias para GDD (base 10°C), Precipitação acumulada, Chuva diária máxima, Dias consecutivos secos (CDD), Média da amplitude térmica diurna (DTR), Desvio padrão amostral da amplitude térmica (DTR std), Radiação solar global acumulada e Umidade relativa média.
2. **9 Features Biofísicas Derivadas**: Mês de observação, Dia juliano do ano (DOY), Fotoperíodo astronômico analítico (duração do dia em horas com clamp de declinação solar), Quociente Fototérmico (PTQ em 30 e 60 dias), Proxy de Déficit de Pressão de Vapor (VPD em 14 e 30 dias), Latitude e Longitude.
3. **3 Encodings Categóricos Numéricos**: Código de agro-ecossistema do arroz (0..6), código de cultivar/variedade (0..185) e código de província administrativa tailandesa (0..57).

## 3. Gestão de Memória e Zero-Copy

- **Sessão Singleton Residente**: O arquivo ONNX (3,2 MB) é carregado na memória RAM uma única vez no boot da aplicação, consumindo aproximadamente 18 MB de memória residente (RSS) e evitando overhead de leitura de disco (I/O).
- **Sincronização Thread-Safe**: A sessão é envelopada em um `std::sync::Mutex<Session>` para garantir segurança em acessos concorrentes sem vazamento de memória ou concorrência descontrolada no runtime C do ONNX.
- **Zero Leakage**: O runtime de inferência processa unicamente arrays estáticos na pilha e no heap local sem alocações dinâmicas repetitivas, permitindo execução contínua 24/7 em nós Raspberry Pi 4 com 1 GB de RAM sem necessidade de reinicialização.

## 4. Conclusão para a Tese de Doutorado / Dissertação

Os resultados confirmam que a substituição de pipelines interpretados em Python por executáveis compilados em Rust com ONNX Runtime C ABI reduz a latência de inferência por predição para o patamar submilissegundo (~21.8 µs), viabilizando previsões fenológicas e geração de recomendações agronômicas instantâneas na borda rural, mesmo sob hardware de baixo custo e restrição energética severa.
