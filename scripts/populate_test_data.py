#!/usr/bin/env python3
"""
Oryza-Elo Edge Engine: High-Fidelity Agronomic Test Data Generator
Populates all SQLite database tables with coherent, physically valid,
and rich randomized data for edge offline development and demonstration.
Zero external library dependencies (uses Python standard library only).
"""

import argparse
import datetime
import json
import math
import os
import random
import sqlite3
import sys
import uuid

# Canonical Agronomic Stages & Translations
STAGES = [
    {
        "code": "10-19",
        "name": "Seedling",
        "phase": "1_Vegetative",
        "stage_key": "seedling",
        "advisory": {
            "pt-BR": {
                "headline": "Fase de Plântula Estabelecida (BBCH 10–19)",
                "plain_text": "Plântulas em enraizamento e emergência foliar. Manter lâmina d'água rasa (1 a 2 cm) para controle térmico sem asfixia radicular.",
                "urgent_warnings": ["Evitar inundação profunda para prevenir afogamento de plântulas jovens.", "Monitorar formigas-cortadeiras e gorgulho-aquático."],
                "management_tips": ["Adubação nitrogenada de base em cobertura.", "Manter saturação constante do solo."]
            },
            "en": {
                "headline": "Seedling Establishment Stage (BBCH 10–19)",
                "plain_text": "Seedlings actively rooting and expanding first true leaves. Maintain shallow water layer (1-2 cm) for thermal control.",
                "urgent_warnings": ["Avoid deep submergence to prevent seedling hypoxia.", "Scout for rice water weevil and early damping-off."],
                "management_tips": ["Apply basal nitrogen starter dose.", "Maintain saturated soil with minimal drainage."]
            },
            "th": {
                "headline": "ระยะกล้าแตกใบอ่อน (BBCH 10–19)",
                "plain_text": "ต้นกล้ากำลังเจริญเติบโตของระบบรากและใบ ควรักษาระดับน้ำตื้น 1-2 ซม. เพื่อควบคุมอุณหภูมิของแปลง",
                "urgent_warnings": ["หลีกเลี่ยงการขังน้ำลึกเพื่อป้องกันต้นกล้าขาดออกซิเจน", "เฝ้าระวังเพลี้ยไฟและหนอนกอข้าว"],
                "management_tips": ["ใส่ปุ๋ยรองพื้นสูตรไนโตรเจน", "รักษาระดับความชื้นในดินอย่างสม่ำเสมอ"]
            }
        }
    },
    {
        "code": "20-29",
        "name": "Tillering",
        "phase": "1_Vegetative",
        "stage_key": "tillering",
        "advisory": {
            "pt-BR": {
                "headline": "Fase de Perfilhamento Ativo (BBCH 20–29)",
                "plain_text": "Emissão acelerada de perfilhos produtivos. Momento ideal para adubação nitrogenada de topo e controle preventivo de plantas daninhas.",
                "urgent_warnings": ["Deficiência de nitrogênio neste estágio limitará permanentemente o número de panículas por m²."],
                "management_tips": ["Aplicar primeira cobertura nitrogenada.", "Elevar lâmina d'água para 3-5 cm para suprimir invasoras."]
            },
            "en": {
                "headline": "Active Tillering Phase (BBCH 20–29)",
                "plain_text": "Rapid tiller emergence determining panicle density per square meter. Critical window for topdress nitrogen application.",
                "urgent_warnings": ["Nitrogen stress during tillering permanently caps productive panicle count."],
                "management_tips": ["Apply topdress urea/ammonium sulfate.", "Maintain 3-5 cm water layer for weed suppression."]
            },
            "th": {
                "headline": "ระยะแตกกอสูงสุด (BBCH 20–29)",
                "plain_text": "ต้นข้าวกำลังแตกหน่อและสร้างกออย่างรวดเร็ว เป็นช่วงเวลาสำคัญในการใส่ปุ๋ยแต่งหน้าและจัดการวัชพืช",
                "urgent_warnings": ["การขาดไนโตรเจนในระยะนี้จะส่งผลให้จำนวนรวงต่อกอลดลงอย่างถาวร"],
                "management_tips": ["ใส่ปุ๋ยยูเรียแต่งหน้ารอบแรก", "รักษาระดับน้ำ 3-5 ซม. เพื่อควบคุมวัชพืช"]
            }
        }
    },
    {
        "code": "40-49",
        "name": "Booting",
        "phase": "2_Reproductive",
        "stage_key": "booting",
        "advisory": {
            "pt-BR": {
                "headline": "Emborrachamento e Iniciação da Panícula (BBCH 40–49)",
                "plain_text": "Panícula em diferenciação no interior da bainha foliar. Fase mais vulnerável a estresse hídrico e brusone (Magnaporthe oryzae).",
                "urgent_warnings": ["ATENÇÃO: Déficit hídrico causa esterilidade severa de espiguetas.", "Condições de alta umidade (>85%) favorecem esporulação de brusone."],
                "management_tips": ["Garantir lâmina de água de 5 a 7 cm sem interrupção.", "Aplicar fungicida preventivo se umidade relativa exceder 85% por 3 dias consecutivos."]
            },
            "en": {
                "headline": "Panicle Initiation & Booting (BBCH 40–49)",
                "plain_text": "Young panicle developing inside leaf sheath. Peak physiological sensitivity to drought and blast disease.",
                "urgent_warnings": ["WARNING: Water stress causes severe spikelet sterility.", "Prolonged high humidity (>85%) accelerates blast sporulation."],
                "management_tips": ["Ensure uninterrupted 5-7 cm flood water.", "Apply preventive blast fungicide if relative humidity exceeds 85% for 3 days."]
            },
            "th": {
                "headline": "ระยะตั้งท้องและสร้างรวงอ่อน (BBCH 40–49)",
                "plain_text": "รวงข้าวเริ่มพัฒนาอยู่ภายในกาบใบ เป็นช่วงที่อ่อนไหวที่สุดต่อการขาดน้ำและโรคไหม้ข้าว",
                "urgent_warnings": ["คำเตือน: การขาดน้ำจะทำให้เมล็ดข้าวลีบอย่างรุนแรง", "ความชื้นสูงกว่า 85% ต่อเนื่องเอื้อต่อการระบาดของโรคไหม้"],
                "management_tips": ["รักษาระดับน้ำในนา 5-7 ซม. ห้ามปล่อยให้น้ำแห้ง", "ฉีดพ่นสารป้องกันกำจัดเชื้อราหากความชื้นสัมพัทธ์สูงติดต่อกัน 3 วัน"]
            }
        }
    },
    {
        "code": "50-59",
        "name": "Heading",
        "phase": "2_Reproductive",
        "stage_key": "heading",
        "advisory": {
            "pt-BR": {
                "headline": "Espigamento e Emersão da Panícula (BBCH 50–59)",
                "plain_text": "Panículas emergindo da bainha foliar. Temperatura ideal entre 25°C e 32°C. Monitorar percevejo-do-arroz.",
                "urgent_warnings": ["Temperaturas diárias superiores a 35°C causam esterilidade polínica.", "Monitorar infestação de percevejo-do-grão."],
                "management_tips": ["Manter lâmina d'água constante para amortecimento térmico.", "Evitar adubações tardias de nitrogênio."]
            },
            "en": {
                "headline": "Heading & Panicle Exsertion (BBCH 50–59)",
                "plain_text": "Panicles emerging from the flag leaf sheath. Optimum temperature 25-32°C. Monitor for rice stink bug.",
                "urgent_warnings": ["High temperatures (>35°C) trigger pollen sterility and floret abortion.", "Scout for rice stink bug panicle feeding."],
                "management_tips": ["Maintain continuous flood for microclimate buffering.", "Do not apply late nitrogen to prevent sheath blight."]
            },
            "th": {
                "headline": "ระยะออกรวง (BBCH 50–59)",
                "plain_text": "รวงข้าวโผล่พ้นกาบใบ อุณหภูมิที่เหมาะสมคือ 25-32°C เฝ้าระวังมวนง่ามและเพลี้ยกระโดดสีน้ำตาล",
                "urgent_warnings": ["อุณหภูมิสูงเกิน 35°C จะทำให้เกสรตัวผู้เป็นหมัน", "เฝ้าระวังแมลงสิงดูดกินน้ำเลี้ยงรวงข้าว"],
                "management_tips": ["รักษาระดับน้ำเพื่อรักษาอุณหภูมิแปลงนา", "งดการใส่ปุ๋ยไนโตรเจนเพื่อป้องกันโรคเมล็ดด่าง"]
            }
        }
    },
    {
        "code": "60-69",
        "name": "Flowering",
        "phase": "2_Reproductive",
        "stage_key": "flowering",
        "advisory": {
            "pt-BR": {
                "headline": "Floração Plena e Antese (BBCH 60–69)",
                "plain_text": "Abertura floral ocorrendo entre 09:00 e 12:00. Não pulverizar durante a antese para não lavar o grão de pólen.",
                "urgent_warnings": ["PROIBIDO pulverizar defensivos agrícolas entre 08:30 e 12:30 para evitar desidratação e lavagem de pólen."],
                "management_tips": ["Pulverizações somente após 15:00 ou início da manhã antes da abertura floral.", "Manter lâmina de 5 cm."]
            },
            "en": {
                "headline": "Anthesis & Flowering (BBCH 60–69)",
                "plain_text": "Active flowering occurring between 09:00 and 12:00. Avoid spray operations during pollination hours.",
                "urgent_warnings": ["MANDATORY: Do NOT spray pesticides between 08:30 and 12:30 to avoid pollen wash and floret damage."],
                "management_tips": ["Schedule necessary treatments after 15:00.", "Maintain shallow water layer of 5 cm."]
            },
            "th": {
                "headline": "ระยะออกดอกและผสมเกสร (BBCH 60–69)",
                "plain_text": "ดอกข้าวกำลังบานและผสมเกสรช่วง 09:00 - 12:00 น. ห้ามฉีดพ่นสารเคมีในช่วงเวลาดังกล่าว",
                "urgent_warnings": ["ข้อควรระวัง: ห้ามฉีดพ่นสารเคมีช่วง 08:30 - 12:30 น. เพราะจะชะล้างละอองเกสร"],
                "management_tips": ["หากจำเป็นต้องพ่นสารเคมี ให้ทำหลังเวลา 15:00 น.", "รักษาระดับน้ำ 5 ซม."]
            }
        }
    },
    {
        "code": "70-89",
        "name": "PreHarvest",
        "phase": "3_Ripening",
        "stage_key": "pre_harvest",
        "advisory": {
            "pt-BR": {
                "headline": "Maturação Leitosa / Pastosa (BBCH 70–89)",
                "plain_text": "Enchimento de grãos em andamento acelerado. Translocação de amido para as panículas.",
                "urgent_warnings": ["Suspender aplicações químicas respeitando o Período de Carência (LMR).", "Iniciar drenagem gradual 10 a 14 dias antes da colheita."],
                "management_tips": ["Manter saturação do solo sem água parada.", "Verificar umidade dos grãos (alvo 20–22%)."]
            },
            "en": {
                "headline": "Grain Filling & Milk/Dough Stage (BBCH 70–89)",
                "plain_text": "Grain filling in full progression. Maximum starch translocation into panicles.",
                "urgent_warnings": ["Adhere strictly to pesticide pre-harvest withdrawal intervals (PHI).", "Begin gradual field drainage 10-14 days prior to harvest."],
                "management_tips": ["Reduce water depth to soil saturation.", "Monitor grain moisture (target 20-22%)."]
            },
            "th": {
                "headline": "ระยะสร้างแป้งและน้ำนม (BBCH 70–89)",
                "plain_text": "เมล็ดข้าวกำลังสะสมแป้งและน้ำนม รวงข้าวเริ่มโน้มลงและเปลี่ยนเป็นสีเหลืองอ่อน",
                "urgent_warnings": ["หยุดพ่นสารเคมีตามระยะปลอดภัยก่อนเก็บเกี่ยว", "เตรียมระบายน้ำออกจากแปลงก่อนเกี่ยว 10-14 วัน"],
                "management_tips": ["ลดระดับน้ำให้เหลือเพียงดินชื้น", "สุ่มตรวจความชื้นเมล็ด (เป้าหมาย 20-22%)"]
            }
        }
    },
    {
        "code": "90-99",
        "name": "HarvestReady",
        "phase": "3_Ripening",
        "stage_key": "harvest_ready",
        "advisory": {
            "pt-BR": {
                "headline": "Ponto de Colheita e Senescência (BBCH 90–99)",
                "plain_text": "Grãos fisiologicamente maduros e dourados (85% a 90% da panícula madura). Umidade ideal entre 18% e 22%.",
                "urgent_warnings": ["Atraso na colheita sob chuva causa quebra no beneficiamento e germinação na espiga."],
                "management_tips": ["Realizar colheita mecânica em horário sem orvalho.", "Encaminhar para secagem imediata (< 24h)."]
            },
            "en": {
                "headline": "Ripeness & Harvest-Ready (BBCH 90–99)",
                "plain_text": "Grains golden and physiologically mature (85-90% panicle straw colored). Target harvest moisture 18-22%.",
                "urgent_warnings": ["Delayed harvest under rain triggers grain cracking, milling loss, and pre-harvest sprouting."],
                "management_tips": ["Harvest during midday dry canopy hours.", "Transfer to grain dryer within 24 hours."]
            },
            "th": {
                "headline": "ระยะสุกแก่พร้อมเก็บเกี่ยว (BBCH 90–99)",
                "plain_text": "เมล็ดข้าวสุกแก่เต็มที่ สีเหลืองทองทั้งรวง (85-90% ของรวง) ความชื้นเหมาะสม 18-22%",
                "urgent_warnings": ["การเกี่ยวช้าเมื่อฝนตกจะทำให้เมล็ดแตกร้าวและข้าวหักเวลารวมโรงสี"],
                "management_tips": ["เก็บเกี่ยวช่วงแดดจัดที่ไม่มีน้ำค้าง", "นำข้าวเปลือกเข้าสู่กระบวนการอบแห้งภายใน 24 ชม."]
            }
        }
    }
]

# 4 Realistic Reference Parcels across Thai Rice Agroecosystems
PARCEL_PRESETS = [
    {
        "id": "talhao-esalq-central",
        "name": "Talhão Central Esalq (Suphan Buri)",
        "rice_variety": "ขาวดอกมะลิ 105",
        "rice_ecosystem": "นาชลประทาน",
        "latitude": 14.4745,
        "longitude": 100.1177,
        "days_ago": 65,
        "area_hectares": 15.0,
        "source": "Davis Vantage Pro2 (Central)"
    },
    {
        "id": "talhao-varzea-chiangmai",
        "name": "Talhão Várzea do Vale (Chiang Mai)",
        "rice_variety": "กข43",
        "rice_ecosystem": "นาชลประทาน",
        "latitude": 18.7904,
        "longitude": 98.9847,
        "days_ago": 40,
        "area_hectares": 8.5,
        "source": "Pessl iMETOS 3.3 (Norte)"
    },
    {
        "id": "talhao-sequeiro-khonkaen",
        "name": "Talhão Sequeiro Nordeste (Khon Kaen)",
        "rice_variety": "กข79",
        "rice_ecosystem": "นาน้ำฝน",
        "latitude": 16.4322,
        "longitude": 102.8236,
        "days_ago": 95,
        "area_hectares": 22.0,
        "source": "Dragino LoRaWAN RS485"
    },
    {
        "id": "talhao-piloto-ubon",
        "name": "Talhão Piloto Experimental (Ubon Ratchathani)",
        "rice_variety": "ขาวดอกมะลิ 105",
        "rice_ecosystem": "นาชลประทาน",
        "latitude": 15.2448,
        "longitude": 104.8473,
        "days_ago": 18,
        "area_hectares": 5.0,
        "source": "NASA POWER Daily Station"
    }
]

CUSTOM_DEVICE_MAPPINGS = [
    {
        "id": "mapping_dragino_sol_nascente",
        "device_name": "Estação LoRaWAN Fazenda Sol Nascente",
        "manufacturer": "Dragino Technology",
        "is_preset": 0,
        "date_col": "Timestamp",
        "date_format": "%Y-%m-%d %H:%M:%S",
        "t_max_col": "AirTemp_Max_C",
        "t_max_unit": "C",
        "t_max_scale": 1.0,
        "t_min_col": "AirTemp_Min_C",
        "t_min_unit": "C",
        "t_min_scale": 1.0,
        "rain_col": "Precip_Accum_mm",
        "rain_unit": "mm",
        "rain_scale": 1.0,
        "rad_col": "Pyranometer_Solar_W_m2",
        "rad_unit": "W/m2",
        "rad_scale": 0.0864, # Converte W/m2 diário para MJ/m2
        "rh_col": "AirHumidity_Pct",
        "rh_unit": "%",
        "rh_scale": 1.0
    },
    {
        "id": "mapping_campbell_pesquisa",
        "device_name": "Estação Micrometeorológica Campbell CR1000X",
        "manufacturer": "Campbell Scientific",
        "is_preset": 0,
        "date_col": "TIMESTAMP",
        "date_format": "%d/%m/%Y",
        "t_max_col": "AirTC_Max",
        "t_max_unit": "C",
        "t_max_scale": 1.0,
        "t_min_col": "AirTC_Min",
        "t_min_unit": "C",
        "t_min_scale": 1.0,
        "rain_col": "Rain_mm_Tot",
        "rain_unit": "mm",
        "rain_scale": 1.0,
        "rad_col": "SlrMJ_Tot",
        "rad_unit": "MJ/m2",
        "rad_scale": 1.0,
        "rh_col": "RH_Max",
        "rh_unit": "%",
        "rh_scale": 1.0
    }
]

EDGE_CONFIG_DEFAULTS = [
    ("nightly_cron_enabled", "true"),
    ("cron_time", "23:59"),
    ("default_locale", "pt-BR"),
    ("biomet_window_days", "60"),
    ("thermal_base_temp", "10.0"),
    ("node_operational_mode", "edge_standalone"),
    ("edge_cache_retention_days", "180"),
    ("alert_push_enabled", "true")
]

def ensure_schema(conn: sqlite3.Connection):
    """Executes initial schema creation if not already present."""
    schema_sql = """
    CREATE TABLE IF NOT EXISTS parcels (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        rice_variety TEXT NOT NULL,
        rice_ecosystem TEXT NOT NULL,
        latitude REAL NOT NULL,
        longitude REAL NOT NULL,
        planting_date TEXT NOT NULL,
        area_hectares REAL,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL
    );

    CREATE TABLE IF NOT EXISTS device_mappings (
        id TEXT PRIMARY KEY,
        device_name TEXT NOT NULL,
        manufacturer TEXT NOT NULL,
        is_preset INTEGER NOT NULL DEFAULT 0,
        date_col TEXT NOT NULL,
        date_format TEXT NOT NULL DEFAULT '%Y-%m-%d',
        t_max_col TEXT NOT NULL,
        t_max_unit TEXT NOT NULL DEFAULT 'C',
        t_max_scale REAL NOT NULL DEFAULT 1.0,
        t_min_col TEXT NOT NULL,
        t_min_unit TEXT NOT NULL DEFAULT 'C',
        t_min_scale REAL NOT NULL DEFAULT 1.0,
        rain_col TEXT NOT NULL,
        rain_unit TEXT NOT NULL DEFAULT 'mm',
        rain_scale REAL NOT NULL DEFAULT 1.0,
        rad_col TEXT NOT NULL,
        rad_unit TEXT NOT NULL DEFAULT 'MJ/m2',
        rad_scale REAL NOT NULL DEFAULT 1.0,
        rh_col TEXT NOT NULL,
        rh_unit TEXT NOT NULL DEFAULT '%',
        rh_scale REAL NOT NULL DEFAULT 1.0,
        created_at TEXT NOT NULL
    );

    CREATE TABLE IF NOT EXISTS weather_records (
        parcel_id TEXT NOT NULL,
        record_date TEXT NOT NULL,
        t_max REAL NOT NULL,
        t_min REAL NOT NULL,
        precipitation_mm REAL NOT NULL,
        radiation_mj_m2 REAL NOT NULL,
        relative_humidity_pct REAL NOT NULL,
        source TEXT NOT NULL,
        created_at TEXT NOT NULL,
        PRIMARY KEY (parcel_id, record_date),
        FOREIGN KEY (parcel_id) REFERENCES parcels(id) ON DELETE CASCADE
    );

    CREATE INDEX IF NOT EXISTS idx_weather_parcel_date ON weather_records(parcel_id, record_date DESC);

    CREATE TABLE IF NOT EXISTS prediction_history (
        id TEXT PRIMARY KEY,
        parcel_id TEXT NOT NULL,
        evaluated_at TEXT NOT NULL,
        macro_phase TEXT NOT NULL,
        granular_stage TEXT NOT NULL,
        confidence REAL NOT NULL,
        is_transitioning INTEGER NOT NULL,
        probabilities_json TEXT NOT NULL,
        advisory_json TEXT NOT NULL,
        metrics_json TEXT NOT NULL,
        FOREIGN KEY (parcel_id) REFERENCES parcels(id) ON DELETE CASCADE
    );

    CREATE INDEX IF NOT EXISTS idx_predictions_parcel_eval ON prediction_history(parcel_id, evaluated_at DESC);

    CREATE TABLE IF NOT EXISTS edge_config (
        key TEXT PRIMARY KEY,
        value TEXT NOT NULL,
        updated_at TEXT NOT NULL
    );
    """
    conn.executescript(schema_sql)
    conn.commit()

def generate_weather_for_day(date_obj: datetime.date, day_idx: int, total_days: int) -> dict:
    """Generates coherent, physically sound tropical rice weather telemetry."""
    # Seasonal wave simulating monsoon fluctuation
    seasonal_phase = (day_idx / 30.0) * math.pi
    wave = math.sin(seasonal_phase)

    # Base temperatures in tropical rice basin
    base_t_max = 32.5 + 2.0 * wave
    base_t_min = 23.0 + 1.2 * wave

    t_max = round(base_t_max + random.uniform(-1.5, 2.2), 2)
    t_min = round(base_t_min + random.uniform(-1.8, 1.2), 2)

    # Physical constraint: T_min must always be strictly less than T_max
    if t_min >= t_max:
        t_min = round(t_max - random.uniform(4.0, 8.0), 2)

    # Rain pattern: monsoon convective showers or dry days
    is_rain_day = random.random() < 0.28
    if is_rain_day:
        precipitation_mm = round(random.uniform(5.0, 45.0) + (15.0 if random.random() < 0.15 else 0.0), 2)
        relative_humidity_pct = round(random.uniform(82.0, 96.0), 1)
        radiation_mj_m2 = round(random.uniform(12.0, 18.5), 2)
    else:
        precipitation_mm = 0.0
        relative_humidity_pct = round(random.uniform(62.0, 80.0), 1)
        radiation_mj_m2 = round(random.uniform(18.0, 24.5), 2)

    return {
        "record_date": date_obj.strftime("%Y-%m-%d"),
        "t_max": t_max,
        "t_min": t_min,
        "precipitation_mm": precipitation_mm,
        "radiation_mj_m2": radiation_mj_m2,
        "relative_humidity_pct": min(100.0, max(30.0, relative_humidity_pct))
    }

def get_stage_for_days_after_sowing(das: int):
    """Maps Days After Sowing (DAS) to physiological rice stage."""
    if das < 20:
        return STAGES[0] # Seedling (10-19)
    elif das < 48:
        return STAGES[1] # Tillering (20-29)
    elif das < 68:
        return STAGES[2] # Booting (40-49)
    elif das < 78:
        return STAGES[3] # Heading (50-59)
    elif das < 88:
        return STAGES[4] # Flowering (60-69)
    elif das < 110:
        return STAGES[5] # PreHarvest (70-89)
    else:
        return STAGES[6] # HarvestReady (90-99)

def build_probabilities_vector(active_stage_key: str, confidence: float) -> dict:
    """Builds a probability distribution across 7 stages summing strictly to 1.0."""
    all_keys = [s["stage_key"] for s in STAGES]
    other_keys = [k for k in all_keys if k != active_stage_key]
    remaining = max(0.001, 1.0 - confidence)

    # Random partition of remaining probability
    weights = [random.uniform(0.1, 1.0) for _ in other_keys]
    sum_w = sum(weights)

    probs = {active_stage_key: round(confidence, 4)}
    distributed = 0.0
    for i, k in enumerate(other_keys):
        if i == len(other_keys) - 1:
            val = round(remaining - distributed, 4)
        else:
            val = round((weights[i] / sum_w) * remaining, 4)
            distributed += val
        probs[k] = max(0.0, val)

    # Final normalization check
    total = sum(probs.values())
    if total > 0:
        probs[active_stage_key] = round(probs[active_stage_key] + (1.0 - total), 4)

    return probs

def populate_database(db_path: str, max_days: int = 75, parcel_count: int = 4):
    """Fills the SQLite database with realistic synthetic data."""
    abs_db = os.path.abspath(db_path)
    os.makedirs(os.path.dirname(abs_db), exist_ok=True)

    print(f"🌾 Conectando ao banco SQLite: {abs_db}")
    conn = sqlite3.connect(abs_db)
    conn.execute("PRAGMA foreign_keys = ON;")
    conn.execute("PRAGMA journal_mode = WAL;")
    ensure_schema(conn)

    cursor = conn.cursor()
    now = datetime.datetime.now(datetime.timezone.utc)
    now_iso = now.isoformat()
    today = now.date()

    # 1. Inserir Custom Device Mappings
    print("\n[1/5] Inserindo perfis e mapeamentos de sensores customizados...")
    custom_inserted = 0
    for dev in CUSTOM_DEVICE_MAPPINGS:
        cursor.execute("""
            INSERT INTO device_mappings (
                id, device_name, manufacturer, is_preset,
                date_col, date_format,
                t_max_col, t_max_unit, t_max_scale,
                t_min_col, t_min_unit, t_min_scale,
                rain_col, rain_unit, rain_scale,
                rad_col, rad_unit, rad_scale,
                rh_col, rh_unit, rh_scale,
                created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                device_name = excluded.device_name,
                manufacturer = excluded.manufacturer
        """, (
            dev["id"], dev["device_name"], dev["manufacturer"], dev["is_preset"],
            dev["date_col"], dev["date_format"],
            dev["t_max_col"], dev["t_max_unit"], dev["t_max_scale"],
            dev["t_min_col"], dev["t_min_unit"], dev["t_min_scale"],
            dev["rain_col"], dev["rain_unit"], dev["rain_scale"],
            dev["rad_col"], dev["rad_unit"], dev["rad_scale"],
            dev["rh_col"], dev["rh_unit"], dev["rh_scale"],
            now_iso
        ))
        custom_inserted += 1
    print(f"  ✓ {custom_inserted} mapeamentos de dispositivos registrados.")

    # 2. Inserir Configurações do Nó de Borda
    print("\n[2/5] Inserindo configurações operacionais da borda (edge_config)...")
    config_inserted = 0
    for key, val in EDGE_CONFIG_DEFAULTS:
        cursor.execute("""
            INSERT INTO edge_config (key, value, updated_at)
            VALUES (?, ?, ?)
            ON CONFLICT(key) DO UPDATE SET
                value = excluded.value,
                updated_at = excluded.updated_at
        """, (key, val, now_iso))
        config_inserted += 1
    print(f"  ✓ {config_inserted} parâmetros de configuração salvos.")

    # 3. Inserir Talhões e Séries Temporais Climáticas
    print(f"\n[3/5] Gerando {parcel_count} talhões com séries biometeorológicas...")
    selected_parcels = PARCEL_PRESETS[:parcel_count]
    total_weather_rows = 0
    total_predictions = 0

    for p in selected_parcels:
        sowing_date = today - datetime.timedelta(days=p["days_ago"])
        sowing_iso = sowing_date.strftime("%Y-%m-%d")

        cursor.execute("""
            INSERT INTO parcels (
                id, name, rice_variety, rice_ecosystem,
                latitude, longitude, planting_date, area_hectares,
                created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                rice_variety = excluded.rice_variety,
                rice_ecosystem = excluded.rice_ecosystem,
                planting_date = excluded.planting_date,
                area_hectares = excluded.area_hectares,
                updated_at = excluded.updated_at
        """, (
            p["id"], p["name"], p["rice_variety"], p["rice_ecosystem"],
            p["latitude"], p["longitude"], sowing_iso, p["area_hectares"],
            now_iso, now_iso
        ))

        # Gerar série climática diária desde a semeadura até hoje
        days_span = min(max_days, p["days_ago"] + 1)
        weather_records = []
        for d_offset in range(days_span):
            cur_date = sowing_date + datetime.timedelta(days=d_offset)
            w = generate_weather_for_day(cur_date, d_offset, days_span)
            weather_records.append((
                p["id"], w["record_date"], w["t_max"], w["t_min"],
                w["precipitation_mm"], w["radiation_mj_m2"],
                w["relative_humidity_pct"], p["source"], now_iso
            ))

        cursor.executemany("""
            INSERT INTO weather_records (
                parcel_id, record_date, t_max, t_min,
                precipitation_mm, radiation_mj_m2, relative_humidity_pct,
                source, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(parcel_id, record_date) DO UPDATE SET
                t_max = excluded.t_max,
                t_min = excluded.t_min,
                precipitation_mm = excluded.precipitation_mm,
                radiation_mj_m2 = excluded.radiation_mj_m2,
                relative_humidity_pct = excluded.relative_humidity_pct
        """, weather_records)
        total_weather_rows += len(weather_records)

        # 4. Gerar Histórico de Inferências Fenológicas (Simulação temporal a cada 7 dias)
        eval_intervals = []
        cur_das = 7
        while cur_das <= p["days_ago"]:
            eval_date = sowing_date + datetime.timedelta(days=cur_das)
            eval_intervals.append((cur_das, eval_date))
            cur_das += 7
        # Adiciona avaliação de hoje se ainda não adicionada
        if not eval_intervals or eval_intervals[-1][0] != p["days_ago"]:
            eval_intervals.append((p["days_ago"], today))

        for das_eval, dt_eval in eval_intervals:
            stage_info = get_stage_for_days_after_sowing(das_eval)
            confidence = round(random.uniform(0.82, 0.96), 4)
            is_transitioning = 1 if das_eval in [19, 47, 67, 77, 87] else 0

            probs = build_probabilities_vector(stage_info["stage_key"], confidence)
            eval_iso = datetime.datetime.combine(dt_eval, datetime.time(23, 59, 0), tzinfo=datetime.timezone.utc).isoformat()

            advisory_envelope = {
                "advisory": {
                    "locale": "pt-BR",
                    "stage": stage_info["stage_key"],
                    "headline": stage_info["advisory"]["pt-BR"]["headline"],
                    "plain_text": stage_info["advisory"]["pt-BR"]["plain_text"],
                    "urgent_warnings": stage_info["advisory"]["pt-BR"]["urgent_warnings"],
                    "management_tips": stage_info["advisory"]["pt-BR"]["management_tips"]
                },
                "all_translations": {
                    "pt-BR": {
                        "locale": "pt-BR",
                        "stage": stage_info["stage_key"],
                        "headline": stage_info["advisory"]["pt-BR"]["headline"],
                        "plain_text": stage_info["advisory"]["pt-BR"]["plain_text"],
                        "urgent_warnings": stage_info["advisory"]["pt-BR"]["urgent_warnings"],
                        "management_tips": stage_info["advisory"]["pt-BR"]["management_tips"]
                    },
                    "en": {
                        "locale": "en",
                        "stage": stage_info["stage_key"],
                        "headline": stage_info["advisory"]["en"]["headline"],
                        "plain_text": stage_info["advisory"]["en"]["plain_text"],
                        "urgent_warnings": stage_info["advisory"]["en"]["urgent_warnings"],
                        "management_tips": stage_info["advisory"]["en"]["management_tips"]
                    },
                    "th": {
                        "locale": "th",
                        "stage": stage_info["stage_key"],
                        "headline": stage_info["advisory"]["th"]["headline"],
                        "plain_text": stage_info["advisory"]["th"]["plain_text"],
                        "urgent_warnings": stage_info["advisory"]["th"]["urgent_warnings"],
                        "management_tips": stage_info["advisory"]["th"]["management_tips"]
                    }
                }
            }

            metrics = {
                "model_version": "catboost_onnx_v1",
                "inference_latency_ms": round(random.uniform(0.019, 0.038), 4),
                "entropy": round(random.uniform(0.18, 0.42), 4),
                "margin": round(confidence - 0.08, 4)
            }

            pred_id = str(uuid.uuid4())
            cursor.execute("""
                INSERT INTO prediction_history (
                    id, parcel_id, evaluated_at, macro_phase, granular_stage,
                    confidence, is_transitioning, probabilities_json, advisory_json, metrics_json
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            """, (
                pred_id, p["id"], eval_iso, stage_info["phase"], stage_info["name"],
                confidence, is_transitioning, json.dumps(probs),
                json.dumps(advisory_envelope), json.dumps(metrics)
            ))
            total_predictions += 1

        print(f"  ✓ {p['name']} ({p['rice_variety']}): {len(weather_records)} dias climáticos, {len(eval_intervals)} inferências.")

    conn.commit()

    # 5. Auditoria de Validação Final
    print("\n[5/5] Executando auditoria de integridade do banco...")
    tables = ["parcels", "device_mappings", "weather_records", "prediction_history", "edge_config"]
    summary = {}
    for t in tables:
        cursor.execute(f"SELECT COUNT(*) FROM {t}")
        summary[t] = cursor.fetchone()[0]

    db_size = os.path.getsize(abs_db) / 1024.0

    print("==================================================================")
    print("🌾  BANCO DE DADOS POPULADO COM SUCESSO (ESTADO DE ALTA FIDELIDADE)  🌾")
    print("==================================================================")
    for t, cnt in summary.items():
        print(f"  • {t:<22}: {cnt:>5} registros")
    print("------------------------------------------------------------------")
    print(f"  Tamanho no disco: {db_size:.1f} KB")
    print(f"  Arquivo: {abs_db}")
    print("==================================================================")
    conn.close()

def main():
    parser = argparse.ArgumentParser(description="Oryza-Elo Test Data Population Script")
    parser.add_argument("--db", default="src/dal/data/local/oryza_elo_edge.db", help="Caminho relativo ou absoluto do banco SQLite")
    parser.add_argument("--days", type=int, default=75, help="Número de dias históricos de telemetria meteorológica")
    parser.add_argument("--parcels", type=int, default=4, help="Quantidade de talhões de referência a criar (1 a 4)")
    args = parser.parse_args()

    populate_database(args.db, args.days, args.parcels)

if __name__ == "__main__":
    main()
