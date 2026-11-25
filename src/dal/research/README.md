# Reprodução dos resultados preliminares

O estudo principal classifica seis estádios do arroz em 2.370 vistorias tailandesas. O [estudo complementar](complementar/README.md) avalia transferência internacional, características de cultivares e arquiteturas recentes. O [catálogo dos dados](../data/tcc/README.md) indica os arquivos usados em cada análise.

## 1. Obter a versão do estudo

Em um terminal Linux com Git e Python 3.13.5, execute os comandos abaixo. A partir do segundo comando, todos os caminhos partem da raiz do repositório.

```bash
git clone --branch v0.2.0 --depth 1 https://github.com/wilsonborba/oryzaelo_engine.git
cd oryzaelo_engine
sha256sum --quiet -c src/dal/data/tcc/SHA256SUMS
python3 src/dal/research/principal/reproduce.py audit
```

A última linha usa a biblioteca padrão do Python para recalcular o macro-F1 a partir das previsões individuais das sete avaliações. Verifique **0,618860 para XGBoost em seis estádios** e **0,760904 para CatBoost em três macrofases**. A meta do TCC é 0,75 em seis estádios.

## 2. Instalar as dependências do estudo

Ambiente de referência: Linux x86-64, CPython 3.13.5. Os pacotes de treinamento e exportação exigem alguns GB de disco. Com o interpretador instalado e o módulo venv disponível:

```bash
python3.13 -m venv .venv
. .venv/bin/activate
python -m pip install -r src/dal/research/requirements.txt
```

A instalação da aplicação Rust e do serviço de borda está no [Quick start do motor](../../../README.md#quick-start). Os experimentos Python podem ser executados em um computador pessoal.

## 3. Reproduzir a parcial principal

```bash
python src/dal/research/principal/reproduce.py train
```

1. O programa verifica a assinatura do CSV do Rice Department e preserva a posição original de cada linha. Registra sete exclusões por coordenadas e 21 por rótulo incerto.
2. Reconstrói as janelas meteorológicas de 7, 14, 30 e 60 dias com as respostas NASA POWER arquivadas. São 44 entradas e 9.480 janelas completas.
3. Ajusta os modelos com as partições do protocolo, grava previsões fora da amostra e exporta o CatBoost numérico para ONNX.
4. Compara a assinatura de `features.csv` e recalcula as sete pontuações. A saída desta execução fica em `.research-run/principal/`. Ali estão também `revision_exclusions.csv`, os mapas de partições e os arquivos `*_oof.csv`.

Consulte o [protocolo e os resultados de classificação](05_revision_v2_reproducibility.md) e as [cinco rodadas previamente medidas no Raspberry Pi 5](06_revision_v2_pi_benchmark.md). A pontuação do XGBoost e os tempos do CatBoost numérico correspondem a modelos diferentes.

## 4. Reproduzir a parcial comparativa

```bash
python src/dal/research/complementar/reproduce.py audit
python src/dal/research/complementar/reproduce.py train
```

O primeiro comando confere as tabelas usando as predições publicadas. O segundo repete os experimentos internacionais e biológicos em CPU. O [roteiro complementar](complementar/README.md) detalha as fontes, o teste temporal e o procedimento separado de retreino neural em GPU.
