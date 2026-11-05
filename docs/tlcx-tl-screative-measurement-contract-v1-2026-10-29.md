# TL・S_Creative 測定契約 v1 — 実装前固定仕様

## 日本語

### 0. 状態・原案への忠実性・優先順位

- 固定日：2026-10-29 JST（原案・TLCX設計は2026-10-28固定）。対象baseline：Epistesys 6.3.2-alpha.3、TLCX設計。実装作業はユーザー指示で一旦中断し、本測定契約を先に固定する。既存差分は保持する。
- 数式・定義の敵対的再監査日：2026-10-29 JST。原案FormulaRevisionは変更せず、操作的測定規約を`finite-formal-v1.4`へ改訂する（v1.1〜v1.3のhashは設計revision chainに保持）。v1.3はv1.1数学再監査のF-01〜03を既に修正済み。今回のv1.4は後続再監査の区間二乗kernelとscheduler novelty policyの識別を追補する。コード実装、校正、TLCX再開は本改訂の範囲外。
- 本書は数学定義・アルゴリズム・適用条件・独立検証・選択影響の規範仕様。実装済み・校正済み・一般意味精度や創造性を実証済みという宣言ではない。
- 原案固定コピーSHA-256：`1bd7540cbb53a907ecd84c1fe0ebbd6bcaf5d495bcf69f75170bf155db386d04`。原文は改稿・repo複製しない。
- `OriginalFormula/F1-v1`と`OriginalFormula/F2-v1`は下記の式・符号・項を保持する。原案は全変数の計算法を定めていない。本書の有限集合・距離・コスト定義は`OperationalMeasurement/finite-formal-v1.4`という補完規約であり、原案が元からこのアルゴリズムを含んでいたとは主張しない。
- v1のD_ctxとv2のD_par、v1のC_matchとv2のQ_parityは別MetricId・別定義。暗黙alias禁止。Q_parity原式は複雑化と同程度の単純化を等しく減点する。非対称化は別FormulaRevisionでのみ導入する。
- 四記録U（Uncertainty）とU_gain（Utility Gain）、創造側D_sem（意味配置の差）とTL semantic loss（保存義務の欠損）を別型・別scopeにする。
- [TLCXロードマップ](epistesys-tlcx-contextual-multilogic-loss-creativity-roadmap-2026-10-28.md)の17〜25、27〜29、38〜41は本契約を参照する。測定規約と数値は実装者がその場で変更しない。

### 1. 数式の正本


### Formula F1 — original S_Creative v1
\[
S_1(z)=\alpha D_{\mathrm{ctx}}(z)+\beta D_{\mathrm{sem}}(z)
-\lambda L_{\mathrm{trans}}(z)+\gamma A_{\mathrm{compat}}(z)
+\delta C_{\mathrm{match}}(z)-\eta N_{\mathrm{known}}(z).
\]
### Formula F2 — original S_Creative v2
\[
S_2(z)=w_1T_{\mathrm{nc}}(z)+w_2G_{\mathrm{struct}}(z)
+w_3R_{\mathrm{gain}}(z)+w_4U_{\mathrm{gain}}(z)
+w_5Q_{\mathrm{parity}}(z)-w_6L_{\mathrm{trans}}(z)
-w_7R_{\mathrm{counter}}(z)-w_8B_{\mathrm{mem}}(z).
\]
\[
T_{\mathrm{nc}}(z)=\sqrt{D_{\mathrm{par}}(z)D_{\mathrm{sem}}(z)}
\,A_{\mathrm{compat}}(z).
\]
\[
G_{\mathrm{struct}}=
a_1\Delta coverage+a_2\Delta compression+a_3\Delta bridge
+a_4\Delta predict-a_5Redundancy.
\]
\[
R_{\mathrm{gain}}=
b_1Preserve+b_2Rebind+b_3Reframe-b_4BreakCost.
\]
\[
U_{\mathrm{gain}}=
c_1\Delta explain+c_2\Delta design+c_3\Delta transfer+c_4\Delta solve.
\]
\[
Q_{\mathrm{parity}}(z)=
\exp\!\left(-\frac{|K(z)-K_0|}{\sigma}\right).
\]

### 2. 必須入力・状態・意味の境界

| 入力 | 必須内容 |
| --- | --- |
| ComparisonScope | 根の問い、元context revision、許可された再構成context-family、hard invariants、可変前提、baseline、GoalProfileRevision/goal policy、unit、固定probe/feature/bridge/corpus集合と各digest |
| Candidate | source/context、typed logical form、明示Delta、model/derivation/artifact、入力・query・結果・再構成の対応 |
| MeasurementBinding | ComparisonScopeId、CandidateId、candidate固有source/target/context revision、producer/transform/result digest、実payload/evidence参照 |
| Probe | 一意ID、logic/fragment、root source property/query、target contract/期待関係、OutcomeKind、ComparatorId、source→target TransformId、unit/dimension、epsilon/range scale、raw AcceptedInputRangeとNormalizationMap（適用時）、正の有理weight、OracleRevision、独立検査範囲、必要な入力根拠 |
| MetricPolicy | MetricId/DefinitionRevision、RootGoalProfileId、active成分、normalize方法、weights、尺度、適用mask、numeric budget、gateとselection mode。幾何schedulerを使う場合はSchedulerNoveltyPolicyIdもpin |
| Observation | query/context/solver/version/options/domain/result/countermodel/proofのpin、取得状態、独立検査状態、scope、データ区分 |

`OutcomeKind`は`BooleanRelation/FiniteRelation/ScalarQuantity/VectorQuantity/RegisteredSemanticOutcome`のいずれか。Boolean/finite relationはsource→targetの許可写像後にrelation成立を照合し、成立0／不成立1のlossとする。数値quantityは登録ComparatorIdで同型・同単位へ写像し、source値に写像Fを適用した値とtarget値の誤差eを算出する。scalarでは`e=|v_target−F(v_source)|`、vectorではComparatorIdが固定するNormIdを用いる。連続lossは`ell=min(1,max(0,e−epsilon_q)/r_q)`、`epsilon_q>=0`は許容差、`r_q>0`は飽和尺度で、両者はeと同じunit。epsilonはprobeをscopeに固定した後に変更しない。eが区間なら単調式を外向き評価する。時刻・座標の変換誤差も同じ式に従う。raw quantityのAcceptedInputRangeとNormalizationBounds/Mapは別設定。saturating mapを使う場合はraw値、適用上下側flag、map revisionを保持し、accepted range外はInvalidInput。ComparatorId、unit、norm、変換が未登録ならUnsupported/NoScalarであり、実装者が既定の距離やtoleranceを選ばない。

重複IDはInvalidInput。hard invariant削除と元context上書きは採点前に拒否する。前提再構成も主目的だが操作権限は探索変数でない。
値の状態は`Exact/Bounded/Estimated/Unknown/NotApplicable/Incomparable/Undefined/InvalidConfig/ResourceExhausted`を区別する。数値状態と、別fieldの`InputBasis`（Given/Assumed/Observed）、`SemanticValidation`、`IndependenceStatus`は直積で保持する。`MeasurementValue`は`Exact{q}`、`Bounded{lo,hi}`、`Estimated{lo,hi,method}`だけを数値payloadとし、その他の状態は数値0でなく型付きreasonを持つ。Exactは宣言された入力値に対する算術精度だけを意味し、外界の真理・正しいparse・証明を意味しない。
Unknownを[0,1]へ埋め込めるのは、そのscopeで量と値域が定義済みで観測だけ不足する場合だけ。Incomparable/Undefined/InvalidConfigをUnknownへ潰さない。ResourceExhaustedは途中の有効boundを併記できるが完了ではない。空分母は原則Undefined/NoApplicableで0でない。ただしK3の有効空集合対とCR-REDUNDANCYの確認済空表現には下記piecewise定義の0を適用する。N_knownの確認済空corpusはmax∅なのでUndefined/EmptyCorpus、未読/未取得corpusはUnknownであり、どちらも0を推定しない。
N/Aは比較scope作成時の適用根拠を要する。候補が情報を落としたためN/Aに変えることは禁止。strict-original式は各参照成分が有効数値／定義済boundでなければNoScalar。N/Aの暗黙0置換・候補別再weight・0×Undefinedによる救済は禁止。係数0と観測値0は別記録。
CandidateEligibilityはEligible/Ineligible/Pendingを別fieldに持ち、Measurementの数値/例外状態と混同しない。前提条件未確認はPrerequisitePending/NoScalar、確認済前提違反はUndefined/PrerequisiteFailedと該当candidate Ineligibleである。
全候補に同じComparisonScopeと評価規約を適用する。候補固有source/target/context revisionはMeasurementBindingへ分離し、全候補で等値を要求するshared scope hashへ含めない。scopeは元context・許可context-family・root-goal対応規約を固定し、candidate membershipとmap-backを検査する。baselineは共通。推奨にはcontext、前提変更、未確定性、最適性scopeを付ける。RootGoalProfileは少なくとも一つの固定GoalMetricIdについてraw unit/type/range/direction、score map、oracle revision、budget、evidence policyを束縛する。scalar raw range[m,M]はM>mを要求し、max goalの標準mapは(v−m)/(M−m)、min goalは(M−v)/(M−m)。区間には単調mapをendpointwise適用する。Exact raw valueがdeclared range外ならInvalidInput/OutOfRange。bounded intervalがrange境界を跨ぐ場合はNoScalar/NeedsEvidence、全区間がrange外ならInvalidInputとし、clipしない。非線形mapや分類score tableは式/tableと係数をProfileRevisionに完全記録する。複数objectiveは別Pareto軸。scalarization weightsやlexicographic representative orderは明示したProfileRevisionに属し、後付けしない。GoalProfileまたは比較可能なgoal observationが無い場合はgoalに基づく推薦を出さない。

### 3. 共通計算kernel

**K1 固定集合weighted assessment**：事前固定P、wi>0、W=Σwi>0。項のloss interval [li,ui]⊆[0,1]から[Σwi li/W,Σwi ui/W]を正確な有理数で返す。許容された保存なら0、確認した不適切な変更／欠落なら1、観測不足は[0,1]。連続量distanceはMetric固有規則を使う。分母と適用maskは候補によらず固定。
**K2 paired gain**：同じP/oracle/budget/unitで項ごとのgi=score_z(i)−score_0(i)を先に作りΣwi gi/Wを返す。score∈[0,1]ならgain∈[-1,1]。丸めた平均同士の差は禁止。入力が不確定ならinterval差、scope不一致ならIncomparable。同一有効観測nodeの自己差x−xは依存関係を保持しExact0とするが入力の不確定性・根拠を消さない。同じintervalを持つ別観測nodeは通常の区間差。
**K3 weighted set distance**：事前固定universe Uとvi>0。A,Bのweighted Jaccard distance=Σ_(A△B)vi/Σ_(A∪B)vi。双方が有効な空集合と確認できた場合だけ0。未抽出集合を空にしない。feature同一性はlogic-specific canonicalizationで決め、raw ID/hash/列挙順を意味距離にしない。A∪B⊆Uを検査し、U外featureはInvalidInput/UniverseMismatchとして拒否する。黙ってdropしない。
**K4 interval算術**：有理数の区間A=[a,b], B=[c,d]に対して、A+B=[a+c,b+d]、A−B=[a−d,b−c]、A×B=[min(ac,ad,bc,bd),max(ac,ad,bc,bd)]。同一不確実量を二乗する専用演算`square([a,b])`は、a<=0<=bなら[0,max(a²,b²)]、それ以外は[min(a²,b²),max(a²,b²)]とする。したがってPoincare norm-squareは座標差ごとのsquareを加算し、genericな区間積x×xで代用しない。有限非空区間集合Xi=[li,ui]のmin enclosureは[min li,min ui]、max enclosureは[max li,max ui]。空集合のmin/maxはUndefined/NoApplicable。−A=[−b,−a]。`abs([a,b])`はa<=0<=bなら[0,max(−a,b)]、それ以外は[min(|a|,|b|),max(|a|,|b|)]。sqrtは下限>=0のみ、単調な有理外向きbound。区間で割る場合、除数が0を含めばUndefined/DivisionByZeroであり、含まなければ逆区間との積。`exp(−[x,y])`（0<=x<=y）は[lowerExp(y),upperExp(x)]。丸めは外向き。式DAG ValueNodeのkeyは(MetricId,DefinitionRevision,MeasurementBindingDigest,ProbeSetDigest)であり、MetricId単独では値の同一性でない。同じcanonical観測nodeの再参照は一度だけ評価し、その自己差だけを依存保持Exact0とする。無関係な同幅intervalは独立nodeとして通常区間演算する。保守的包含は誤差確率や統計的独立性ではない。
**K5 solver truth bridge**：古典logicではΓの整合性を先に検査。Γ=UNSATならContextInconsistentとして古典採点を止める。Γ=SATならΓ∧¬φのUNSATはsemantic rule上φ支持、Γ∧φのUNSATは¬φ支持、両方SATならundetermined。これはsolverの報告から導かれる論理関係であり、未検証solver出力を独立確認済みの事実にしない。結果は`SemanticResult`（ContextInconsistent/Supported/Refuted/Undetermined/Unknown/Conflict）と`ValidationBasis`（SolverReported/ModelRechecked/ProofChecked/FiniteExhaustive）に分ける。SAT witnessは完全なmodelを元の正確な式に独立再評価した場合だけModelRechecked。UNSATは登録proof checkerによるproof検査、または宣言有限domainの独立全列挙を通った範囲だけProofChecked/FiniteExhaustive。これらがなければSolverReportedのまま、independent validationへ数えない。timeout/unknownは推測しない。複数solverの一致も共通parser/translator等の共有依存を検査しない限り独立 corroborationではない。ASPはstable-model semantics、brave/cautious、explicit/default negationをpin。時相はfinite/infiniteとtrace量化をpin。四値Both/Neitherを古典Falseにしない。SATはentailmentでも最適性でもない。
**K6 canonical記述長**：有限typed termのprefix-free encoding EについてK=8×len(E(term)) bits。形式はtag:u8＋payload。Bool=canonical u8(0/1)、Integer=sign:u8＋最短unsigned magnitudeのULEB128 byte長＋little-endian bytes（zeroは正sign・1byte zero）、Symbol=UTF-8 byte長ULEB128＋bytes、Apply=operator Symbol＋arity ULEB128＋child E列、Binder=固定binder tag＋sort Symbol＋body E（束縛変数はde Bruijn index）。tag表/ULEB128最短性/grammar revisionを固定。非意味的ID、receipt、cache、source注釈、整形空白は含めない。意味のあるchild順序をsortしない。alpha-renaming以外の同値化はlogicで検査済みの規則だけ。graph→term変換が未対応ならUnknown。KはproxyでありKolmogorov complexityではない。K0=K(z0)>0、σ/κのunitはbits。
**K7 数値精度**：基本量は上限付きBigRational（符号・分子・正分母、GCD正規化）。参照上限は分子/分母それぞれ65,536bits、probe/feature各100,000、corpus10,000件、bridge100,000辺。precision pは1<=p<=256のProfile固定整数（default reference 64）。超過はResourceExhausted。sqrtはp-bit gridの整数二乗比較isqrtで[q/2^p,(q+1)/2^p]、分子・分母がとも完全平方のときだけ有理Exact。K7はMetric固有NormalizationMapを先に適用した後の数値kernel。normalized leaf [0,1]の外はInvalidInputで自動clipしない。raw inputのsaturationはCR-DESIGN等、Mapに明示登録した成分のみ許可し、AcceptedInputRange、raw値、SaturatedLow/Highを保持する。
exp(−x),x>=0はx=0だけExact1。r=min{r>=0:x/2^r<=1/2}は有理比較の繰返しで求め、浮動小数logは使わない。y=x/2^r。S_m=Σ_(k=0)^m(-y)^k/k!とし、S_(2j+1)を下界、S_(2j)を上界とする。termをterm×y/kで正確更新しgap<=2^{−(p+r+16)}まで求める。上限をP=p+r+16ビットの格子へ変換する際、lo=floor(L*2^P)/2^P、hi=ceil(U*2^P)/2^P。r回の非負区間二乗はlo'=floor(lo²*2^P)/2^P、hi'=ceil(hi²*2^P)/2^Pで各回外向き。最終width<=2^−pを確認、未達ならPを32増やし丸め前の有理Taylor boundから最大3回再計算。参照値p=64/r<=64/term<=256。r>64はshift/Taylor前に拒否。超過は有効bound付きResourceExhausted、underflowをExact0にしない。K interval [kL,kU]ならdmin=0 if kL<=K0<=kU、そうでなければmin(|kL−K0|,|kU−K0|)、dmax=max(|kL−K0|,|kU−K0|)。Q intervalは[lowerExp(dmax/σ),upperExp(dmin/σ)]であり、数学的supremumは閉区間がK0を含めば1、含まなければ1未満（ただし包含は可行candidateの存在証明ではない）。外向き数値上界は非交差でも丸めにより1となり得る。dmin>0、x=dmin/σなら`upper_Q=min(upperExp(x),1/(1+x))<1`とする（e^x>1+x）。K0を含むenclosureのupper=1はouter boundのみでExact1ではない。端点だけで最大を求めない。[0,u]の下限0をExact0と誤称しない。

IndependenceStatusはUnassessed/SharedDependency/ReportedSeparation/CheckedSeparation {checked_layers,gaps,evidence_refs}を区別する。登録oracleや別runのみではCheckedSeparationへ昇格しない。constructorはscopeに束縛された別checker/lowering・gold出所・dataset分割・共有依存台帳を検査し、実際に確認した層だけ記録する。caller自由text/自己申告はReportedSeparation止まり。真正性は設定trust rootに対する観測receipt検査で別管理し、key所有者や人間の独立性を推定しない。未確認層はgapとして保持する。

### 4. parameter・profile・未校正の扱い

原式係数は27個：F1の6、F2の8、aの5、bの4、cの4。σは別scale parameter。TL axis weights、κ_M/κ_B、feature/probe weights、gate thresholdsも別設定としてProfileDigestに含める。
係数はfiniteな非負有理数（本運用規約の追加制約）。原案は係数和1を要求しない。非正規化値は係数から値域を導出してそのまま返す。全係数0なら代数値は0だがDegenerateWeights、OperationalRankingProfileでは拒否。σ/κ_M/κ_B/K0は正、未設定はMissingParameter、無効値はInvalidConfig。epsilonや1で救済しない。
`reference-uniform-test-v1`はF1各1/6、F2各1/8、a各1/5、b/c各1/4、σ=K0、κ_M=K0、κ_Bはfixture指定。これはgolden試験用の明示的な好みであり校正済production weightではない。
TLの適用axis maskとweightは元contextから比較前に一度確定し、適用axisで一度正規化して全候補で固定。active weight全0はUndefined。適用候補ごとのdrop/reweightは禁止。
`OriginalFormula/F1-v1`・`F2-v1`はstrict score、`TLCX/JointPareto-v1`は別意思決定profile。原式を参考表示しても改良式を原式と表示しない。未校正はDeclaredPreference/Uncalibratedと表示し、正確な算術と研究的妥当性を混同しない。

**K7補足**：x=n/d>=0のsqrt grid整数はq=isqrt(floor(n·2^(2p)/d))。両端二乗で下界/上界を確認する。分子・分母とも完全平方なら有理Exactを返せる。負sqrt入力、非有限値、NormalizationMap後の[0,1] leaf逸脱はInvalidInputでclipしない。raw値のsaturationはMetric固有MapとAcceptedInputRangeの両方が宣言された場合だけ許す。

### 3A. kernel補足 — 実装者が選んではいけない部分

- K6 tag表：0x01 Bool、0x02 Integer、0x03 Symbol、0x04 Apply、0x05 Binder、0x06 BoundVar。BoundVar payloadは最短ULEB128 index。Binder payloadはkind:u8（0 forall、1 exists、2 lambda）＋E(Symbol sort)＋E(body)。ApplyのoperatorもE(Symbol)。その他tagはUnsupported。indexは現在のbinder depth未満を要する。0x03のsymbol比較はUTF-8 byte exact、暗黙Unicode正規化なし。別canonicalizationは別profile。
- nodeの子数/長さを読んでから上限・残byte・depthを検査する。最大depth256、term byte長16MiB。cycleを拒否する。Integer signは0=positive/1=negativeのみ、他はInvalidInput。magnitude length>=1、len>1の最上位byte0、negative zero、過長ULEB、Bool 2、未束縛index、trailing byteを拒否。BundleはApply(Symbol `bundle-v1`, [semantic_term, decoder_term, dictionary_term])の固定3子。不要decoder/dictionaryも明示Bool(false)で表す。
- K6 golden：Bool(false)=`01 00`（16bits）、Symbol("a")=`03 01 61`（24bits）、Integer(0)=`02 00 01 00`（32bits）、Apply("f",[Bool(false)])=`04 03 01 66 01 01 00`（56bits）。これらはterm encodingのvectorでありBundle全長とは別。
- K3はsorted canonical feature mergeでO(|A|+|B|)、K1/K2はO(n)、archive全照合はO(corpus×feature)、DAG評価O(nodes+edges)、DijkstraはO((V+E)log V)。全入力と中間big integerにもbudgetを適用し、count/lengthのchecked arithmeticを使う。
- 固定reference formal fragment：有限命題n<=16の全割当列挙、有限typed relation（総domain tuple<=100,000）、明示有限trace長<=256、有限4値query、ground ASP（model cap256）。SMT/ASPは登録fragmentのみ。cap超過/無限領域の証明なし/未対応logicはUnsupported/NeedsEvidenceで、近似を形式検証にしない。拡張時はfragment revisionを増やす。
- formula DAGは循環・unknown MetricId・重複定義・scope不一致を拒否する。同じnodeを別係数で原式が参照することは許すが、shared-input依存を消さない。productionで文字列名を比較せずenum/typed IDと固定registryを用いる。
- 算出profileはProbeScoreKindをBinaryOracleまたはNormalizedQuantity {unit,m,M,direction}へ限定。自由rubricを使うprofileはEstimatedとして別にpinする。登録されていないoracle、feature extractor、bridge correspondenceを実装者が推測して埋めない。

### 5. 成分カタログの読み方

以下の40契約は、TL補助量・四記録・原式全成分と下位成分を網羅する。各契約にはユーザー指定の5欄を必須とする。共通入力/K1〜K7/例外処理も各契約の一部である。『独立検証』は今後実装する試験要求であり、本書がその実行を完了したことを意味しない。

#### TL-RECON — reconstruction loss

- **数学的定義・値域・単位**：許可された逆写像H(Y)で元の保護featureを再構成できない割合。固定P_reconにK1を適用。L_recon∈[0,1]、無次元。byte一致率とは別量。
- **計算アルゴリズム**：target semantic payloadだけを隔離decoderへ渡す→再構成→固定feature oracleで照合→各失敗1/成功0/未観測[0,1]→K1。decoderの辞書・model・補助情報はpinし、Kの圧縮costにも含める。
- **適用条件**：復元対象、許容同値関係、decoderが宣言された有限課題。原sourceをsidecarやID検索から読むdecoderは禁止。target自身が全sourceを含む場合0はあり得るが、理解・圧縮の証明ではない。
- **独立検証方法**：独立decoderと小入力全列挙、1feature削除の手計算、source参照を遮断した隔離試験。恒等0、必須feature削除>0、候補ID変更不変を検査。
- **選択への影響**：TL-TOTALのactive軸として増加は損失増。必須復元propertyの欠損は別hard gate。高値probeを再lowering/別表現探索へ送り、0からValidatedを発行しない。

#### TL-SEM — semantic preservation loss

- **数学的定義・値域・単位**：固定必須意味probeの保存関係に違反する重み割合。各probeはequivalence/soundness/completenessのいずれかを明記。K1、[0,1]、無次元。創造側D_semとは異なる。
- **計算アルゴリズム**：source/target queryを固定mappingでlowerし、各Γについて先にconsistencyを検査する。contextがSATならΓ∧¬φがUNSATのときだけφをSupported、Γ∧φがUNSATのときだけRefuted、両方SATならUndetermined。equivalenceは両向きの含意queryがともにSupported、soundnessは各target-claimedφ_tの逆写像がsourceでSupported、completenessは各Required source-claimedφ_sの写像がtargetでSupportedとなることを検査する。ΓがUNSATならContextInconsistent、結果がConflict/Unknown/timeout/Unsupportedならlossを確定せずUnknown等のreasonを保持する。違反を独立確認したprobeのみ1、完全passのみ0。
- **適用条件**：logic signature/量化/domainが固定され、意味対応が検査可能なfragment。異なるlogic間は検証済bridgeを要する。自由文の完全理解・全意味空間への一般化は対象外。
- **独立検証方法**：共通parserを使わない小モデル列挙、独立type/effect checker、手作成goldで否定・量化・scope・例外を変異。Γ矛盾、Both/Neither、solver timeoutを成功としない。
- **選択への影響**：保存欠陥はTLと該当obligationへ戻す。hard claim違反を高い新規性で相殺しない。意味的に新しいが許可された変更はDeltaへ記録し、必須保存違反でない限りTL欠陥にしない。

#### TL-TIME — temporal preservation loss

- **数学的定義・値域・単位**：固定時間probeの順序・duration・絶対時刻等の保存違反。二値関係は0/1。数値probeは`e_q=|v_target−F_q(v_source)|`（同一canonical unit）から`ell=min(1,max(0,e_q−epsilon_q)/r_q)`を算出する。r_q>0と許容差epsilon_q>=0は同一unit。K1により[0,1]無次元へ集約。
- **計算アルゴリズム**：ClockId/TimeScale/Origin/Unit/Precision/ReferenceIdを検査し、まず共通canonical unitへ厳密換算する。Fはcanonical source time→canonical target time。absolute instantは`F_abs(t)=a*t+b`、elapsed durationは`F_dur(Δt)=a*Δt`でbを加えない。intervalはstart<endの非空`[start,end)`、逆転/空intervalはInvalidInput、瞬間eventは別型。beforeはendA<startB、meetsはendA=startB。order/duration/absolute timestampは別probe。誤差区間の後にepsilon/r_q式で外向き計算。
- **適用条件**：校正とclock対応がある場合。共通unit換算後の残余`t'=a*t+b`はa>0なら順序を保存する。duration保存は残余a=1、absolute-time保存は残余a=1かつb=0（または明示校正済origin map）を要する。a<0の順序反転は許可Deltaとして別宣言されない限り保存違反。許可Deltaはhard temporal obligationの削除ではない。時間順序は因果ではない。
- **独立検証方法**：独立rational時刻checker、単位変換不変、before/meets境界、時刻反転、clock不一致、欠測を0にする変異を検査。tolerance/precisionを区間へ伝播。
- **選択への影響**：active時間lossをTLへ返し、時間obligationを修復。未校正clockはUnknown/Incomparableで保留し、空間や創造利得では埋めない。

#### TL-SPACE — spatial preservation loss

- **数学的定義・値域・単位**：固定空間probeのtopology/包含/方向/metric保存違反。関係は0/1。数値probeはComparatorIdで固定した`e_q=ComparatorId(F(source),target)`から`ell=min(1,max(0,e_q−epsilon_q)/r_q)`を算出する。epsilon_q>=0とr_q>0は同じunit。K1後は[0,1]無次元。
- **計算アルゴリズム**：FrameId/次元/軸/origin/unit/handedness/object対応/TransformClassを検査。許可写像後、ComparatorIdで距離・角度・包含等のerrorを評価。vector NormId、角度wrap rule、epsilon、r_qはpolicy revisionで固定し、未登録ならUnsupported。変換/測定誤差の区間を伝播してからlossへ写す。
- **適用条件**：rigid/similarity/affine等で保存性が異なる。pixel↔world、2D↔3Dは校正なしに比較しない。鏡映が許可ならhandedness差自体を誤りにしない。探索用非ユークリッド幾何とは別。
- **独立検証方法**：独立geometry checkerと小図形手計算、rigid/unit変換、相似で距離非保存、object再ラベル、depth欠落の敵対vector。変換不確実性を伝播。
- **選択への影響**：active空間lossと個別欠陥を返す。無校正入力は不明のまま。探索geometryの近さで空間保存を認定しない。

#### TL-TOTAL — L_trans

- **数学的定義・値域・単位**：L_trans=Σ_(a∈A)ρ_a L_a/Σρ_a、ρ_a>0、Aは比較前固定の適用軸。値域[0,1]、無次元。原案には集約式がなく本規約で補完。
- **計算アルゴリズム**：元contextからaxis maskと重みを固定→各軸の有効性確認→K1と同様に区間加重和。W=0はUndefined。N/A軸はscope作成時のみ除外し、候補ごとには除外しない。
- **適用条件**：軸の正規化・対象・保存義務が同一scopeで比較可能な場合。strict F1/F2が参照するL_transはこのTLReductionPolicy IDへ束縛。
- **独立検証方法**：独立rational計算、軸順序不変、全軸0/1、重み比例不変、1軸unknown、空分母、候補都合のmask変更拒否を検査。
- **選択への影響**：F1では−λL、F2では−w6Lとして一度反映。hard軸の閾値は集約と別に検査し、平均で重大欠陥を隠さない。JointParetoでは独立loss次元。

#### TL-PATH — end-to-end preservation

- **数学的定義・値域・単位**：元source→最終targetの同じ保護probeを直接再検査したL_end∈[0,1]。edge lossの単純和ではない。D ledgerはunique(root obligation, property, revision)で欠陥を同定。
- **計算アルゴリズム**：各edgeに対応写像とwitnessを保存→map compositionを検査→最終targetでTL各軸を再計測→同一欠陥をroot keyでまとめる。重複した1欠陥を複数lossとして加算しない。
- **適用条件**：対応写像がcompose可能なpath。途中で失われ後段で再取得した場合、最終保存と途中欠損履歴を別記。再取得の証拠がなければ回復を推測しない。
- **独立検証方法**：小path全列挙、identity edge挿入不変、同一欠陥二重化不変、drop→restoreの証拠有無、逆方向TLが別値になる例を検査。
- **選択への影響**：最終回答gateはL_endと個別hard義務を使う。局所低lossや累積loss低下だけで出力を解放しない。path履歴は再探索箇所を選ぶ。

#### TL-MODAL — modal_likeness

- **数学的定義・値域・単位**：固定required capability集合に対して実際に対応可能と確認した重み割合、K1のsuccess版、[0,1]無次元。真理確率でない補助適合度。
- **計算アルゴリズム**：registryの対応宣言と該当revisionの実行probeの両方を照合。宣言のみはunobserved。成功1、確認不対応0、不明[0,1]を固定分母で平均。
- **適用条件**：選択する表現・backendが同一課題のcapability要求と比較可能な場合。未対応と未観測を別に保持。
- **独立検証方法**：registry虚偽、backend差替え、probe切断、固定分母の独立再計算。slot存在だけで1にならないことを検査。
- **選択への影響**：表現選択の候補と探索予算に使う。適合度が高くてもTL保存や操作許可や出力commitを認定しない。

#### REC-D — four-record Defect

- **数学的定義・値域・単位**：Dは保護義務に反した事象の有限集合でありscalarでない。key=(root obligation, property, source/target revision)、各事象に対応TL軸と根拠。countのunitは件だがcount=TLではない。
- **計算アルゴリズム**：保存probeの確認違反からのみ生成→root keyでdedup→span/transform/observationsへリンク。未観測はDに確定せずREC-Uへ。
- **適用条件**：許可変更Deltaと保存義務が先に分離されている変換。必須義務の消去はDeltaで正当化しない。
- **独立検証方法**：手作成1欠陥、重複path、許可再構成、未観測を混ぜた独立ledger照合。
- **選択への影響**：該当obligation修復とhard gateへ接続。創造利得から欠陥事実を消さず、countだけで候補順位を作らない。

#### REC-DELTA — four-record permitted change

- **数学的定義・値域・単位**：Deltaは元／候補context、変更対象、変更前後、理由、許可根拠、map-backを持つtyped edit集合。件数は距離/利得そのものではない。
- **計算アルゴリズム**：可変premise/frameworkへの差分を抽出し、hard setとの交差を拒否→各論理における変更意味を記録→Dctx/Dpar/Dsemへ入力。元contextは不変。
- **適用条件**：前提再構成を主目的に含める探索。ただし根の問いとhard invariantsを変えるには新ComparisonScopeを要する。操作権限の再構成は禁止。
- **独立検証方法**：独立diffと小context全列挙、単なるID変更、hard義務削除、定義域変更を隠した候補を検査。
- **選択への影響**：再構成枝を正当に比較するための根拠。新しい文脈の答えを元文脈の証明として扱わない。

#### REC-U — four-record uncertainty

- **数学的定義・値域・単位**：Uは未観測/曖昧/比較不能/未対応/未検証のgap集合。確率やU_gainでない。件数とscopeは報告可能だが1個のconfidenceに縮約しない。
- **計算アルゴリズム**：各metric/probeの未確定状態とreasonをunique keyで記録し、必要証拠と次actionを保持。状態の違いをfalseへ潰さない。
- **適用条件**：全変換で必須。定義自体がない場合と観測だけ不足する場合を別reasonにする。
- **独立検証方法**：timeout/不適用/分母0/矛盾/未対応の各入力で異なる状態が残り、成功receiptが発行されないことを検査。
- **選択への影響**：Clarify/Hold/追加probe/探索配分へ接続。Uが減ったというだけで意味利得や正解を認定しない。

#### REC-G — four-record gain

- **数学的定義・値域・単位**：Gは固定baseline相対の検証された保存/再結合/能力変化のevent集合。各eventはmetric、primitive feature、paired observationを参照。数式G_structとは別。
- **計算アルゴリズム**：観測gainと予測gainを別tagで登録→shared primitive IDで重複を検出→CR各項へ集計。削除された必須条件をgainにしない。
- **適用条件**：同じroot goal、baseline、固定評価集合への対応がある比較。異なるtaskの容易さは共通mapなしに利得としない。
- **独立検証方法**：手作成paired例、同じbridgeを別名で重複、predictedをobservedへ偽装した例を独立検査。
- **選択への影響**：共同最適化の利得根拠と代替解説明へ接続。重複依存を表示し、別の独立証拠票として加算しない。

#### CR-DCTX — D_ctx

- **数学的定義・値域・単位**：元contextと再構成contextの可変premise/fact feature集合のK3距離。[0,1]無次元。logicの公理/推論規則signatureはD_parに分離。
- **計算アルゴリズム**：scopeにpinしたuniverseで両集合をcanonical化→差集合/和集合の重み和をK3計算。変更不能featureの差は先にhard violation。
- **適用条件**：可変前提を型付き表現にできるscope。公理系自体の差をDctxへ暗黙混入しない。
- **独立検証方法**：独立set計算、ID再ラベル/順序不変、A={a,b},B={b,c}の距離2/3、未抽出≠空を検査。
- **選択への影響**：F1の+α項。α>0かつgate合格なら距離増はraw score増。他成分・gate次第で採用されない。新しさは正しさではない。

#### CR-DPAR — D_par

- **数学的定義・値域・単位**：元と候補framework signature集合（logic、axiom、ontology sort、inference rule、演算意味）のK3距離。[0,1]無次元、Dctxと別。
- **計算アルゴリズム**：signatureの対応を明示しK3。二つの同名演算でも意味が異なれば別feature。mere renameは対応規約で同一。
- **適用条件**：同一root goalへのbridgeがあるframework再構成。例えば通常整数とF2では『2』のlabel/元対応を宣言する。別体系の解を整数の解と同一視しない。
- **独立検証方法**：小公理signature全列挙、演算意味変更、型変化、名前だけ変更の独立oracle。
- **選択への影響**：F2のTncに入る。Dpar=0ならTnc=0だが他のgainやS2全体は0とは限らない。

#### CR-DSEM — D_sem

- **数学的定義・値域・単位**：固定nonmandatory意味配置queryに対するbaseline/候補のcanonical outcomeが異なる重み割合、[0,1]無次元。必須保存queryはTL-SEMへ。
- **計算アルゴリズム**：各queryを同一scopeで評価→relation/assignment等のtyped outcome比較→違い1/同じ0/不明[0,1]のK1。真理値を違うlogic間で無写像比較しない。
- **適用条件**：query universe、outcome同値関係、変更許容部分が固定。高距離を損失とも有用性とも自動認定しない。
- **独立検証方法**：独立小model、同義再ラベル0、有効な意味再配置>0、必須関係dropはTLにも記録するvector。
- **選択への影響**：F1 +β、F2 Tncの入力。意味新規性の探索を促すがhard保存や有用性gateを通過する必要がある。

#### CR-ACOMP — A_compat

- **数学的定義・値域・単位**：固定integration/bridge queryが既存体系と正しく接続する成功重み割合、[0,1]無次元。embedding類似度でない。
- **計算アルゴリズム**：型/意味/roundtrip/実使用を各queryの登録oracleで検査→成功1、失敗0、不明[0,1]→K1 success。hard queryは別gate。
- **適用条件**：source/targetとinterface契約がpinされ、接続の成功条件が実行または形式検査可能な場合。
- **独立検証方法**：独立interface driver、bridge逆写像、型は合うが意味が違う例、slotだけ存在する例を検査。
- **選択への影響**：F1 +γ、Tncへ乗算。A=0はTncを0にする。他の高得点はhard接続義務違反を相殺しない。

#### CR-CMATCH — C_match

- **数学的定義・値域・単位**：C_match=1/(1+|K−K0|/κ_M)、κ_M>0 bits。[0,1]内で正、無次元。原案v1の未定義計算法をこの有理proxyで補完し、Qとaliasしない。
- **計算アルゴリズム**：K/K0をK6で計算し正確有理演算。intervalは|K−K0|の最小/最大を計算後単調写像、K0を跨ぐと上限1。
- **適用条件**：同じencoding revision・bits・baseline、κ_M pin。Kが不明なら厳密point scoreなし。
- **独立検証方法**：独立fraction計算、差0で1、差κで1/2、±同差対称、跨ぎ区間の最大1。
- **選択への影響**：F1 +δ。対称な複雑性釣合いを評価する運用規約。単純化を別に評価したい場合はJointParetoのcost/gainに分離し原式を書き換えない。

#### CR-NKNOWN — N_known

- **数学的定義・値域・単位**：固定非空archive内の最大construction-feature類似度max_y(1−K3(Fz,Fy))。[0,1]無次元。人類全体の既知性ではない。
- **計算アルゴリズム**：corpus revision・feature universe・canonicalizerをpin→全件の類似度→最大。確認済空archiveはUndefined/EmptyCorpus、未読archiveはUnknown、抽出未対応はUnsupported。既知重複1、確認した全件非重複でのみ小値。
- **適用条件**：比較可能なconstruction archiveがある場合。頻度、出典の有名さ、LLMの記憶確信で代用しない。
- **独立検証方法**：独立corpus照合、完全複製1、ID変更不変、空corpus Undefined、部分検索の最大下界と未探索上界。
- **選択への影響**：F1 −η。corpus追加で値が変わり得るため異revisionは再計測する。既知候補を禁止するgateではない。

#### CR-TNC — T_nc

- **数学的定義・値域・単位**：原式sqrt(D_par D_sem) A_compat、[0,1]無次元。いずれかdistance=0またはA=0で厳密0。
- **計算アルゴリズム**：共有metric nodeを参照しK7のsqrt boundとK4積。完全平方以外をf64だけでExactとしない。入力欠落はNoScalar/Bounded方針に従う。
- **適用条件**：三成分が同一comparison scopeで定義済み。非ユークリッドschedulerの距離でDを置換しない。
- **独立検証方法**：独立高精度計算と平方でbound検証。Dpar=1/4,Dsem=1,A=1/2なら1/4。0、単調性、未定義を検査。
- **選択への影響**：F2 +w1。新規性と接続性を同時に要求するが、有用性やhard義務の代用品ではない。

#### CR-COVERAGE — Δcoverage

- **数学的定義・値域・単位**：固定coverage-validation query集合で説明/解決可能と確認された割合のpaired改善K2、[-1,1]無次元。
- **計算アルゴリズム**：各queryは実導出・model・execution等のoracleを持つ。baseline/候補のpass 1/fail0/unknown intervalを同じ予算で取得しK2。
- **適用条件**：scope固定の説明/解決対象。queryを増減した場合は新scope。一般化研究の最終holdoutとは別。
- **独立検証方法**：独立有限queryの全列挙と手計算。baseline[1,0],candidate[1,1]なら1/2。難例除外を拒否。
- **選択への影響**：F2のG_structで+a1。単なる新規claim数をcoverage増とせず、実oracleのpass増を要求。

#### CR-COMPRESSION — Δcompression

- **数学的定義・値域・単位**：(K0−Kz)/max(K0,Kz)∈[-1,1]無次元、Kはbits。必須意味同値/保存が検査済みの場合だけ有効な圧縮利得。
- **計算アルゴリズム**：K6で比較対象と必要decoder/dictionaryを同じ規約で符号化→total bit長を算出→比。比較情報をdecoderへ隠すexternal lookupはcost外にできない。保存未確認はPrerequisitePending/NoScalar、失敗はUndefined/PrerequisiteFailedかつcandidate Ineligible。未検証の短縮率を有効gainとしない。
- **適用条件**：共通required情報を表現するself-contained artifact。sourceを捨てて短くした候補はgainの有効域外。
- **独立検証方法**：別encoder、可逆な手計算例、dictionary込み長、必須情報削除、zero denominator拒否。
- **選択への影響**：G_struct +a2。K減少を喜ぶ前に保存gate。Qの対称減点と圧縮利得の競合は原式上の明示された好みで、黙って相殺規則を追加しない。

#### CR-BRIDGE — Δbridge

- **数学的定義・値域・単位**：固定cross-domain queryが検証済bridgeで利用可能となる割合のK2改善、[-1,1]無次元。edge数でない。
- **計算アルゴリズム**：各queryにsource/target/goal対応・type/semantic/使用oracleを登録→baseline/候補の成功判定→K2。重複名のedgeは同一primitive。
- **適用条件**：対応を検査できる論理・domain間。無対応の異taskはIncomparable。
- **独立検証方法**：独立bridge driver、小有限モデル、edgeだけ追加してquery失敗の負例、同一bridge複製不変。
- **選択への影響**：G_struct +a3。新しい使える接続を評価。接続の存在だけでpermissionや全semantic truthを昇格しない。

#### CR-PREDICT — Δpredict

- **数学的定義・値域・単位**：Brier(z0)−Brier(z)、Brier=Σwi(pi−yi)^2/W、pi∈[0,1],yi∈{0,1}。[-1,1]無次元。多classは別revision。
- **計算アルゴリズム**：固定prediction-validation dataと観測label、予測時点、model revisionをpin。予測をlabel取得前に凍結。paired squared error差をK2集計。
- **適用条件**：label/予測の真正性と時点がある二値課題。学習・探索に用いたvalidationと最終独立holdoutを別管理。未観測labelはUnknown。
- **独立検証方法**：独立rational Brier計算、完全正解0error/逆予測1error、leakage検査、予測を事後書換えたreceipt拒否。
- **選択への影響**：G_struct +a4。探索中validation上の改善であり、独立holdoutを再利用して性能を主張しない。

#### CR-REDUNDANCY — Redundancy

- **数学的定義・値域・単位**：固定役割別semantic unit multisetでΣ_class max(0,n_class−1)/Σ_class n_class、[0,1]無次元。必須反復はrole別unitなので別class。
- **計算アルゴリズム**：独立検証された意味同値だけでcanonical classを作りcount。空の有効表現は0と明示し、抽出不足はUnknown。表層同語だけで統合しない。
- **適用条件**：semantic role/同値classが検査可能な場合。安全のための別箇所の警告等を自動削除しない。
- **独立検証方法**：独立class列挙、同義duplicate、同語異役割、同値未証明、空と未抽出の差。
- **選択への影響**：G_struct −a5。identityでもRedundancyが0とは限らずbaseline Gstructは−a5Redundancy0。削除actionは別権限。

#### CR-GSTRUCT — G_struct

- **数学的定義・値域・単位**：F2原下位式a1Δcoverage+a2Δcompression+a3Δbridge+a4Δpredict−a5Redundancy。無次元。範囲[−Σ1..5 ai, Σ1..4 ai]。
- **計算アルゴリズム**：5子nodeと係数をpinしK4で評価。全gainを[0,1]へclipしない。shared evidence dependency groupを表示。
- **適用条件**：5成分すべて有効値/定義済boundの同一scope。strict formulaで不適用を0にしない。
- **独立検証方法**：独立線形代数、負gain/負項の符号、baseline非zero、全端点からrange包含を検査。
- **選択への影響**：F2 +w2。各子の係数積w2aiが感度。単一scalar以外に各利得/欠陥を残し、重複した根拠を独立票扱いしない。

#### CR-PRESERVE — Preserve

- **数学的定義・値域・単位**：固定source structural propertyの保存成功重み割合K1、[0,1]無次元。差分でなく絶対値、恒等で通常1。
- **計算アルゴリズム**：各propertyをsource/target対応で検査→成功1/失敗0/不明interval→K1 success。hard subsetを別gate。
- **適用条件**：保存対象と許容同値が固定された構造変換。syntax同形だけでsemantic preservationとしない。
- **独立検証方法**：独立property checker、identity1、関係1本破壊、再ラベル不変、trace不足unknown。
- **選択への影響**：R_gain +b1。恒等のraw Rgain=b1を許す。利得差を見たい場合は別delta_Sを計算する。

#### CR-REBIND — Rebind

- **数学的定義・値域・単位**：固定source unitのうち結合先/役割が実際に変わり、有用なmap-backと使用probeが成立した割合、[0,1]無次元。
- **計算アルゴリズム**：unitごとの前後bindingを比較→rename除外→再結合のtype/semantic/goal使用を独立oracle確認→成功K1。変化なし0。
- **適用条件**：binding/役割と共通goal対応を検査可能な再編成。意図した再結合と単なる壊れた参照を分ける。
- **独立検証方法**：小graphの独立列挙、rename0、useful rebind1、dangling reference失敗。
- **選択への影響**：R_gain +b2。Preserve/Reframeとの重なりは原式の明示された多面評価でdependency表示。JointParetoではprimitive重複を除く。

#### CR-REFRAME — Reframe

- **数学的定義・値域・単位**：固定perspective queryのうちbaselineで不成立、明示frame変更＋検証済map-backで候補が説明可能になった重み割合、[0,1]無次元。
- **計算アルゴリズム**：各queryのbaseline failureを確認→frame変更をDeltaで確認→candidate passとroot対応を検査→指標I(base_fail AND mapped_pass)をK1。base unknownはunknown。
- **適用条件**：元問いへ戻せる視点変更がある場合。単に問いを易しくした変更は不適格。
- **独立検証方法**：独立query oracle、frame変更なし0、正当な視点転換、root goalを変更した偽改善。
- **選択への影響**：R_gain +b3。新しい視点の利得を評価するが元条件を満たしたという主張にはmap-backを要する。

#### CR-BREAK — BreakCost

- **数学的定義・値域・単位**：固定optional legacy compatibility/propertyの確認破壊率K1、[0,1]無次元。hard義務や操作権限はcost外のgate。
- **計算アルゴリズム**：legacy property suiteを同一条件で実行→違反1/保存0/不明interval→K1。既存の壊れたbaselineも別測定し勝手に免責しない。
- **適用条件**：soft互換性のtrade-offを許可したscopeのみ。試験数や重みは候補で変えない。
- **独立検証方法**：独立compatibility driver、1破壊の手計算、hard破壊をsoftに移した改ざん拒否。
- **選択への影響**：R_gain −b4。soft破壊への負の好みであって、high gainでpermission/安全義務を買う仕組みではない。

#### CR-RGAIN — R_gain

- **数学的定義・値域・単位**：原式b1Preserve+b2Rebind+b3Reframe−b4BreakCost。範囲[−b4,b1+b2+b3]無次元。
- **計算アルゴリズム**：4子nodeを同一scopeから取得しK4。Preserveが絶対値であることを保ち、名前gainだからと自動中心化しない。
- **適用条件**：各子の適用と比較scopeが成立する場合。
- **独立検証方法**：独立線形計算、identity R=b1（Rebind/Reframe/Break=0）、Breakの符号、共有根拠を検査。
- **選択への影響**：F2 +w3。再編成の多面的価値。raw identityの非zeroは仕様どおりで、改善はS(z)−S(z0)で別表示。

#### CR-EXPLAIN — Δexplain

- **数学的定義・値域・単位**：固定explanation-validation queryの正しい導出/説明成立scoreのpaired gain K2、[-1,1]無次元。
- **計算アルゴリズム**：queryごとにcheckable derivation checkerを登録しpass1/fail0を取得。自由proseをLLMが採点する場合はEstimatedかつrubric/model/judge固定、Exact形式領域と同じ保証にしない。
- **適用条件**：導出・説明のoracleがある課題。自然言語全体の説明能力ではなく固定queryに限定。
- **独立検証方法**：別実装のproof/derivation checker、正しい結論に誤導出、scope抜け、説明文だけ長い負例。
- **選択への影響**：U_gain +c1。説明の長さ・断言の強さでは上がらない。未知の採点は追加検証へ送る。

#### CR-DESIGN — Δdesign

- **数学的定義・値域・単位**：固定design acceptance suiteのscore改善K2、[-1,1]無次元。各scoreはbinaryか、事前固定m<MとAcceptedInputRangeを持つquantityをnormalizeし、large-is-betterはclip((x−m)/(M−m),0,1)、small-is-betterは1−clip((x−m)/(M−m),0,1)。m/Mはsaturation scaleでAcceptedInputRangeそのものではない。
- **計算アルゴリズム**：AcceptedInputRangeを先に検査。range内だがNormalizationBounds m/M外の値はMapの明示saturationとしてclipし、raw値と上下clip flagを残す。AcceptedInputRange外はInvalidInput。small/large directionはscope固定。unit整合、同じresource budgetと環境でbase/candidateを実測しpaired差。未実測design提案はpredicted。
- **適用条件**：実行・simulation・形式oracleが定義された設計課題。目標値/上下限を候補に合わせ変更しない。hard acceptanceは別gate。
- **独立検証方法**：独立acceptance driver、境界値m/M、unit変換、AcceptedInputRange外の拒否、domain内normalization-bound saturationの記録、片方だけ予算を増やす不正を検査。
- **選択への影響**：U_gain +c2。設計実効性を評価。性能や資源値の単位を失わず元値も保存する。

#### CR-TRANSFER — Δtransfer

- **数学的定義・値域・単位**：事前分離したtransfer-validation familyのscore改善K2、[-1,1]無次元。scoreはbinaryまたはCR-DESIGNの固定量正規化。
- **計算アルゴリズム**：source訓練/探索familyとの分割digestを確認→同じ転移budgetでbase/candidateを実行→query別差。最終research holdoutはこの選択入力へ戻さない。
- **適用条件**：root goalと転移対応が明示され、familyの分割が漏洩しない場合。別taskへの無根拠一般化は禁止。
- **独立検証方法**：独立split検査、重複case検出、別driver、zero-shot/few-shot budget差、holdout漏洩の負例。
- **選択への影響**：U_gain +c3。観測したtransfer-validationでの候補選択のみ。普遍的転移能力や独立研究評価を主張しない。

#### CR-SOLVE — Δsolve

- **数学的定義・値域・単位**：固定root-goal solution suiteで正解/実行可能性/目的品質scoreの改善K2、[-1,1]無次元。各probeのbinaryまたはCR-DESIGN正規化を固定。
- **計算アルゴリズム**：候補の実model/導出/program出力を独立oracleへ渡す→hard feasibilityを先に検査→同じgoal/query/budgetでscore差。形式に合うだけの回答を解決成功にしない。
- **適用条件**：正解/feasibility/品質oracleが登録されたscope。originalと再構成contextの答えは明示bridgeが必要。
- **独立検証方法**：小domain全探索optimal oracle、誤答の形式適合、条件を削った簡単な解、solver未完了を検査。
- **選択への影響**：U_gain +c4。実問題解決を選択へ反映。実行時間・token量は別資源単位として同時報告し混ぜない。

#### CR-UGAIN — U_gain

- **数学的定義・値域・単位**：原式c1Δexplain+c2Δdesign+c3Δtransfer+c4Δsolve。無次元、範囲[−Σci,Σci]。
- **計算アルゴリズム**：4子nodeをK4で評価、各dataset roleとevidence basisを保存。REC-Uと型を共有しない。
- **適用条件**：同一baseline/root goalへの定義済score。四課題が全て適用しない場合strict F2はNoScalar、別profileへ明示切替するまで欠項を補完しない。
- **独立検証方法**：独立線形計算、負の改善、全端点、重なるquery/evidence依存、N/A masking負例。
- **選択への影響**：F2 +w4。predicted/Estimatedだけならその範囲の提案でありObserved gainへ昇格しない。

#### CR-K — K, K0

- **数学的定義・値域・単位**：K(z)=8 len(E(z)) bits、K0=K(z0)>0、整数。実用記述長proxyであり最小program長/計算不能なKolmogorov complexityではない。
- **計算アルゴリズム**：K6のtag grammarでsemantic termと必要decoder/dictionaryを同じself-contained Bundle termに符号化。共通runtimeはProfileDigestで固定し双方に等しく扱う。元参照IDで情報を外へ逃さない。
- **適用条件**：有限acyclic typed termへlower可能な表現。任意Rust実行意味や自然言語意味の完全canonical化ではない。
- **独立検証方法**：独立byte encoder/decoder、gold hex、最短ULEB拒否、alpha rename不変、ordered child入替え差、dictionary漏れ。
- **選択への影響**：C_match/Q/Δcompressionの共有node。Kを減らすため保存義務を削った候補はgate失敗。K自体は意味真理を判定しない。

#### CR-QPARITY — Q_parity

- **数学的定義・値域・単位**：原式exp(−|K−K0|/σ)、σ>0 bits。有限Kで(0,1]無次元。±同差の単純化/複雑化を等しく扱う。
- **計算アルゴリズム**：K6のKと固定σからK7でdmin/dmaxを計算。lower=lowerExp(dmax/σ)。K区間がK0を含めばouter upper=1（Exact1はKがexactly K0の証拠がある場合だけ）。K0を含まずdmin>0ならupper=min(upperExp(dmin/σ),1/(1+dmin/σ))。underflow/roundingの0/1を数学的Exact値にしない。
- **適用条件**：同一encoding/unit/baseline/scale。非対称版を使うなら別FormulaRevision、原式名を使わない。
- **独立検証方法**：独立高精度exp、差0で1、差σでe^-1、±対称、K0跨ぎ、tiny positiveのbound。
- **選択への影響**：F2 +w5。対称釣合いの好み。単純化利得はΔcompressionと共同表示し、対称原式を都合よく変更しない。

#### CR-RCOUNTER — R_counter

- **数学的定義・値域・単位**：固定非空adversarial probe suiteでの確認failure重み割合K1、[0,1]無次元。反例の発見件数でも一般的反例確率でもない。
- **計算アルゴリズム**：固定ケースを同じsolver/resourceで実行し、反例は別checkerで再評価。failure1/pass0/unobserved[0,1]。試行0/予算0/探索未実施はUnknown。完走suiteでfailure0なら当該有限suite上のみ0。
- **適用条件**：scope/攻撃範囲/quantificationが明示されたsuite。存在反例1件で全称hard claimを否定できる場合は別hard gate。
- **独立検証方法**：独立countermodel evaluator、隠した難例、timeout、同じ反例duplicate、全称claimの1反例。
- **選択への影響**：F2 −w7。高値は候補の再検査/修復へ。小値を安全性や全domain正解と解釈しない。

#### CR-BMEM — B_mem

- **数学的定義・値域・単位**：全admissible nonsecurity conversionを列挙した閉じた有限graph上のminimum bridge cost Cを`B_mem=C/(C+κ_B)`で正規化する。κ_B>0は同じtyped cost unit。C∈[0,∞]、B_mem∈[0,1]。Cが区間なら単調mapを両endpointへ適用する。完全な閉グラフでpathなしの場合だけ1とDisconnectedを併記。
- **計算アルゴリズム**：全admissible nonsecurity conversionを列挙した閉じた有限graph、始点終点、nonnegative edge costをpin。costがintervalなら全下限weightでDijkstraを一度、全上限weightで一度実行し、`C=[minPath(lower),minPath(upper)]`をouter boundとする。Graphのedge registryが閉じている場合に限り、path不存在をDisconnectedとする。κ_B変換は単調に両endpointへ適用する。同cost pathはcanonical transition-payload sequenceでtie-breakしopaque IDは使わず、同一content/evidence path群は一つのcost witnessへ統合。pathは独立検査。timeoutはUnknown＋有効bound。早期探索失敗からdisconnectを推測しない。
- **適用条件**：実導入労力等の非security障壁。costはtyped work-unit等固定。edge registryが未閉鎖/未観測ならpath不存在はUnknown/NeedsEvidence。権限不足、禁止、hard安全境界をedge costとして買わない。
- **独立検証方法**：独立Bellman-Ford/小graph全path、cost0、pathなし、負edge拒否、cost単位の比例変換、path検証。
- **選択への影響**：F2 −w8。接続労力のsoft好み。Denied/Unknown permissionはスコアに関係なく独立execution gate。

#### CR-V1 — S_Creative v1

- **数学的定義・値域・単位**：Formula F1そのまま。無次元preference index、範囲[−(λ+η),α+β+γ+δ]。確率でも普遍的創造性でもない。
- **計算アルゴリズム**：6成分・6係数・定義revisionをpinしK4。raw S1とbaseline S1(z0)を別評価してdelta S1を返す。identityでA=C=1、固定archiveがbaseline構造を含んでN=1、Dctx=Dsem=L=0ならγ+δ−η。
- **適用条件**：全参照成分が有効な同一scope、係数/原式revisionが固定。欠項の暗黙0補完はしない。
- **独立検証方法**：独立rational式評価、符号/変数置換変異、27係数台帳、identity、NoScalarと全0係数degenerateを検査。
- **選択への影響**：OriginalScore-v1ならgate後にS1最大化（boundsは保守比較）。追加TL罰を入れる場合は別policyで二重係数を展開。標準JointParetoの補助表示とは区別。

#### CR-V2 — S_Creative v2

- **数学的定義・値域・単位**：Formula F2そのまま。G範囲[glo,ghi]、R[rlo,rhi]、U[ulo,uhi]なら下界w2glo+w3rlo+w4ulo−w6−w7−w8、上界w1+w2ghi+w3rhi+w4uhi+w5。無次元。
- **計算アルゴリズム**：全8項と子nodeを同一DAGで評価。raw baselineとdelta S2を別計算。identity Redundancy0、Preserve1、Q1、他0ならS2(z0)=w3b1+w5。Redundancy0>0なら−w2a5Redundancy0も残す。
- **適用条件**：厳密F2の全成分が定義されるfinite-formal比較。使えない成分を落としたreduced scoreは別profile/名前でありF2完成とはしない。
- **独立検証方法**：独立calculator、全子の符号/weight変異、baseline非zero、区間境界、cutで選択が変わるvector。
- **選択への影響**：OriginalScore-v2はgate後S2最大化。JointParetoは別policyでlossとgainを比較する。S2内のTLを再び別のTLとして無自覚に減点しない。

### 6. 選択契約 — 計測と採用を最後まで接続する

1. 入力identity・schema・現在authority/evidence・hard obligationを先に検査する。hard失敗は当該候補の不適格、観測不足はNeedsEvidence/Hold。探索提案を保持することと実行許可は別。
2. 同一ComparisonScopeだけ比較する。再構成contextも主目的として残すがroot goalへのmap-backがなければ別frontier。元contextの解と再構成解を混ぜて『普遍的最適』としない。
3. `JointPareto-v1`は固定`RootGoalProfile`のgoal objective（最大）、active TL各軸（最小）、適用primitive gain（最大）、soft cost（最小）のvectorを使う。RootGoalProfileはGoalMetricId、raw unit/type、direction、oracle revision、scope/baseline、score mappingを固定する。scalar normalizationは固定境界と単調写像を指定し、複数goalは別Pareto軸で保持する。複数goalをscalarizeする場合は別revisionと明示utility profileを要する。hard feasibilityはscalar utilityで相殺しない。GoalProfileがない場合はNoRecommendation（候補/代替の提示は可）で、暗黙goal scoreを生成しない。primitiveはCR-COVERAGE/COMPRESSION/BRIDGE/PREDICT/PRESERVE/REBIND/REFRAME/EXPLAIN/DESIGN/TRANSFER/SOLVE、soft costはCR-REDUNDANCY/BREAK/RCOUNTER/BMEM。適用maskはscope作成時固定。RootGoalScoreとCR-SOLVEは絶対目標値とbaseline-relative改善という別量になり得る。同じGoalObservation primitiveを使う場合はdependency groupを共有し、独立証拠票として数えない。両方をPareto軸にする場合はDeclaredOverlapにその意味を明記する。S1/S2・Gstruct/Rgain/Ugainをさらに同時加算しない。
4. 最大化成分でA.lower>=B.upper、最小化でA.upper<=B.lowerが全成分で成立し、少なくとも1成分でstrictならAが保守的にdominates。重なる区間は未確定で、平均値だけで全順序にしない。全Exact値が一致する場合のみEquivalentInScope（真の意味同値とは別）。同じ区間boundだけならSameBounds/IndistinguishableAtCurrentResolutionであり実値等値としない。unknown boundのみの候補を黙って捨てずexplorationへ残す。
5. frontierから、評価可能なら1推奨＋重要代替解を返す。単一scalar goalのdefault representative keyはgoal lower bound最大→L_trans upper bound最小→declared primitive順のlower gain/upper cost。multi-goalはGoalProfileがpinしたrepresentative order/scalarizationがある場合だけそのpolicyで代表を選び、無ければNoRecommendationと全重要候補を返す。tie表示keyはCandidateId/順序/非意味timestampを除いた完全canonical content＋未解決義務＋policy/evidence lineage bytes。key/evidenceまで同一のduplicateは一つの代表表示へmergeし全candidate aliasesを保持。同じcontentで証拠が異なるものは証拠群を併記し、ID順で代表を選ばない。どの代表も宣言policyでの選択であり、未知の真の優越性の証明ではない。別scope候補は重要代替として併記する。
6. 上限付きloss gateはupper<=τだけPass、lower>τはFail、それ以外NeedsEvidence。最低gain gateはlower>=τでPass、upper<τでFail。必須保存のτは0、soft thresholdはprofile必須parameterであり既定値を推測しない。
7. OriginalScore-v1/v2を明示選択した場合だけraw original scoreを最大化する。strict scalar scoreの区間でcandidate Aをcertified winnerとするのは、lower(A)>upper(B)が他の全eligible Bに成立する場合だけ。区間が重なる場合はtop setを未確定として残す。完全一致のExact値はcanonical display tie-breakだけで代表表示でき、優越性を主張しない。J=aL−bS2なら実効TL係数はa+bw6（v1ならa+bλ）。式を展開して記録し、原式内TLと別TLが同一量なら意図した重複かをpolicyで明示。標準ParetoではこのJを使わない。
8. Geometryはpriority proposalのみ。参照discrete schedulerはpriority=(1+u)(1+n)/(1+c_cost)。uは適用scalarの各bound幅をdeclared range幅で割った最大（固定値域幅0の成分は0）、nはscopeでpinした`SchedulerNoveltyPolicyId`による[0,1]の離散proxy、c_cost=min(1,declared estimated query-slot cost/B)。参照profileは既存挙動を維持する`DistanceToFrontierUnion-v1`で、n=K3(proposed branchのcanonical構造feature集合, validated frontier全体の同feature集合のunion)。`NearestVerifiedCandidate-v1`はn=min_y∈F K3(proposed features, features(y))を計算する別policyであり、和集合距離とのaliasではない。current defaultはunion policyのまま維持し、nearest policyを有利と理論例だけから結論しない。両policyが候補順を逆転し得るため、比較前にPolicyIdを固定し、同一scope・等budgetで多様性/coverage/再現性を評価する。feature universe/cost-estimator revisionをpinし、u,n,c_cost∈[0,1]。未定義入力/空active setはcanonical round-robin。B>0はfixed oracle-query dispatch slots。1slotは1query（timeoutも消費）で、logical-step/wall/memory capはprofile固定。ceil(B/8) slotsを後述のfairness cursor付きSchedulerCanonicalKey順round-robin、残りはpriority lower bound順。同priorityは同SchedulerCanonicalKey順。query完了ごと再計算。未校正な配分で、全branch観測を保証しない。GeometryPolicyが次節の登録要件を満たす場合だけ別の`n_geo`を使う。Geometry/distillationはproposal→離散検証→再測定なしにsemantic metricを書換えない。
8a. 入力/計算に区間があればpriorityもK4で有界区間にする。残りpriority枠はpriority lower boundの降順でdispatchし、完全tieはSchedulerCanonicalKey順とする。この規則は有限budget下のdeterministic scheduler policyであり、潜在priorityの順位や回答utilityの確率的優越性を主張しない。

8b. `SchedulerCanonicalKey`はcandidateのcanonical typed content（opaque CandidateId/ProbeId/BranchId、serialization order、非意味timestampを除く）＋未解決obligation/probe payload＋Definition/Oracle/Policy revisions＋evidence payload digestから作る。証拠keyはkind/result/scope/issuer/backend/policy/dependency等意味fieldを含め、opaque receipt/observation ID、配列順、transport timestampは除く。raw ID/arrival orderはrankingに使わない。keyとevidence lineageが完全一致するduplicate workは一つのschedule groupへ統合し、別候補への出典参照は保持する。`BudgetSeriesId`はTaskRun/ComparisonScope/SelectionPolicyに一度割り当て、state recoveryでも再利用するopaque idであり、priority orderに使わない。`FairnessState`は同一BudgetSeriesIdに束縛し、last-served key、cursor、SchedulerCanonicalKey順に整列したfrontier canonical digest/revision履歴を保存する。frontierが更新されても、last keyが残ればその次から、消えた場合は以前のkeyのcanonical successorから周回し、新keyはcursor位置の後へcanonical順に挿入する。scope/policyが変われば別series。cursorがない/不正なら最初はcanonical最小keyから開始し、cross-epoch fairnessはUnanchoredと報告する。各公平枠query後にcursorを更新する。これは有限予算での偏りを抑える規約で、全候補を必ず処理する保証ではない。ID relabelだけで優先順・代表解が変わらないことを試験する。
9. 値変化が必ず勝者を変えるとは主張しない（係数0、gate失敗、他成分支配等）。各active componentで、他条件同一・非zero感度・十分marginのfixtureを用意し、measurement→frontier/order→推奨または次queryが実際に変わることを検査する。cut時はNeedsEvidenceまたは明示的NoRecommendationとなり、cached winnerを返さない。

### 6A. 探索用Poincare距離と適用限界

探索geometryは物理空間・TL真値・因果の測度ではなく、観測済み状態遷移の優先配分提案である。登録済み`PoincareBallDistance-v1`はdimensionless座標で、曲率−κ_curv（κ_curv>0）、領域`B={x∈R^d:κ_curv||x||_2^2<1}`上の距離を

`d_κ(x,y)=acosh(1+2κ_curv||x−y||_2^2/((1−κ_curv||x||_2^2)(1−κ_curv||y||_2^2)))/sqrt(κ_curv)`

とする。κ_curv=1は標準unit Poincare ball distance、一般κはx'=sqrt(κ_curv)xへのscalingで得る。`d_κ>=0`、対称、`d_κ(x,y)=0 iff x=y`、三角不等式を満たす。κ_curv、novelty scale s_geo>0、muと全座標をGeometryPolicyDigestに含める。同じdimension d>=1のfinite dimensionless rational coordinatesまたはbounded intervalsだけを受理し、固定境界margin 0<μ<1を要求する。区間座標はcoordinate box全体が`κ_curv||x||²<=1−μ`を満たすときだけDomainValid。未証明はNeedsEvidence、領域違反はInvalidInput。丸めfloatはbit-patternのexact binary rationalまたは誤差区間として入力し、外部連続座標の意味的真値へ昇格しない。境界越え、NaN、未有限、overflowは成功値へclipしない。

K8内部数値規約：p_out=64をGeometryPolicyに固定する。各座標endpointから、次の非負有理量`t`と恒等式を用いる。

`t=κ_curv*||x-y||_2^2/((1-κ_curv*||x||_2^2)(1-κ_curv*||y||_2^2))`

`acosh(1+2*t)=2*atanh(sqrt(t/(1+t)))`

K4で座標入力から非負有理t区間[tL,tU]を外向き算出する。`||x-y||²`および各`||x||²`はK4専用の`square`規則で、座標差の区間を成分ごとに二乗してから和を取る。汎用区間積`X×X`で同じ未知座標を二乗し、負の下限を作ってはならない。各有理endpoint t_iについてp_inner=80から開始し、K7で`q_i=sqrt(t_i/(1+t_i))`を外向きboundする。endpoint距離は`dκ(t_i)=2*atanh(q_i)/sqrt(κ_curv)`である。atanh q は`S_N(q)=Σ_(k=0)^N q^(2k+1)/(2k+1)`の下界S_N(qL)と上界`S_N(qU)+qU^(2N+3)/((2N+3)(1−qU²))`で包む。N=0..256を増やし、qとsqrt(κ_curv)の区間演算を含む各endpoint距離enclosureの数値幅（上界−下界）が<=2^-p_outになれば、そのendpoint計算は収束したとする。全体の外向き出力はtL側の距離下界からtU側の距離上界で作る。Nを増やしてもqのsqrt rounding widthは減らないため、数値幅が未達ならp_innerを32ずつ上げ、qとsqrt(κ_curv)から再計算し、最大256までとする。入力tL/tU間の意味的不確実性は最終output interval widthに残し、各endpointの数値幅と混同しない。qU=1またはendpoint slack未達ならp_innerを増やして再計算する。precision/resource capまで解消しない場合はvalid outer boundがあればResourceExhausted付きで保持し、なければNoScalar。距離をExactや∞へ作らない。t=0のexact点はd=0。

この距離だけでは状態→座標は決まらない。EmbeddingPolicyIdはtransition-graph snapshot/edge evidence、feature-to-node correspondence、embedding objective、初期化、optimizer/version/seed、dimension、curvature、novelty scale s_geo、stopping rule/budget、held-out relation checksをpinする必要がある。これらが揃わない座標はUnsupportedで、ID順やLLMの座標案から補わない。検証済frontier Fが空ならround-robin。非空で`d_near=[dL,dU]=min_(y∈F)d_κ(x_proposal,y)`が得られたら、`n_geo=[1−upperExp(dL/s_geo),1−lowerExp(dU/s_geo)]`（s_geo>0、曲率距離と同じdimensionless unit）を探索優先度専用に使う。これは校正済意味的新規性でなくEstimated proposalである。priority intervalはK4で伝播し、そのlower bound順、完全tieはSchedulerCanonicalKey順でdispatchする。これは優越性の判定ではない。Directionalityは対称distanceから推定せず、登録済み`DirectedTransitionCost`を別に使う。Poincare distanceの原型は[Nickel & Kiela, NeurIPS 2017](https://proceedings.neurips.cc/paper/2017/file/59dfa2df42d9e3d41f5b02bfc32229dd-Paper.pdf)のball modelに基づく。本仕様はmetric計算を固定するが、実際のEmbeddingPolicy/production consumerが未登録なら非ユークリッド経路は未接続であり、discrete schedulerへfallbackする。

### 7. Rust実装手順とwire／資源契約

本節は将来の実装手順であり、今回sourceを変更しない。

| 所有層 | 実装する純粋kernel／責務 | 禁止 |
| --- | --- | --- |
| substrate | Digest32/typed IDs、checked binary、bounded rational codec、span/budget | JSONを内部正本、raw struct dump、unsafe decode |
| core | ComparisonScopeId、ProbeId、MetricId、独立state enum | score→Authority変換 |
| tl | K1〜K7、四軸、四記録、TLReductionPolicy、Formula DAGのchecked入力 | 未知0化、単一heuristic値のtruth化 |
| world | S1/S2 pure evaluator、Pareto、K8/registered GeometryPolicyによる探索priority、代替解 | geometryによる検証skip |
| solvers | 論理別結果、model/proof/countermodel payload | SAT/含意/最適性混同 |
| runtime | 同一snapshotでproducer→測定→consumer→Coding/Media/出力を編成 | doctor成功をproduction接続扱い |

実装順：metric registry→scope検査→rational/interval→K1/K2/K3→K6→K7→各leaf→S1/S2→Pareto/gate。K8/Geometryは別world-sliceの依存として、登録済coordinate producerとconsumerがある場合だけ後続接続する。→production consumer→cut/失効/再開。leafを先に実装し、formula evaluatorが成分計測を再帰的に勝手生成しない。

必須profileのslice開始時に、必要oracle一覧・payload schema・positive/negative入力を固定し、Unsupportedを返すだけで必須実装の完成としない。

各MetricIdは固定enum discriminantとDefinitionRevisionを持つ。Rust型をMeasurementValue（ExactRational/BoundedRational/Estimated）、EvaluationState、InputBasis、SemanticValidation、IndependenceStatusへ分離する。private constructorでunit/scope/rangeを検査し、reported値からchecked値へDeserializeで直行しない。既存`lc631-substrate::reported::Measurement`はreport容器であり、本測定の検証済型ではない。

各kernelは`Result<MeasurementReport, MeasurementError>`を返し、matchは全variant明記、`_ => 0`、`unwrap_or_default`、暗黙f64 cast、saturating arithmeticでの成功継続を禁止する。MissingInput/Unavailable/Unsupported/Conflictをreasonとして保持する。Intervalはfinite rational endpointsとlo<=hiをprivate constructorで保証。exactのlabelは計算精度だけに付け、入力観測や意味検証は別field。

binaryはTLCX-03のchecked frameを使用し、各metricごとに数値rational・state tag・scope/policy/evidence参照を保存する。既存JSONはadditiveな外部viewのみ。rationalはsign:u8、正分母、最短magnitude、GCD=1、zero正符号。巨大traceは1MiB以下chunk、logical frame16MiB、既存public fieldを消さない。生のRust layoutやpointerは保存しない。

cache keyはsource/target/context/baseline/probe/corpus/oracle/logic/encoding/policy/evidence revision全てを含む。どれか変わればStale→再測定。旧scoreをCompletedとして復元しない。active lease/receipt/journal/model/benchmarkを不要cacheと同一視しない。今回cacheを削除しない。暫定metric cacheは固定quotaと所有leaseを持ち、evictionは再計算可能objectだけ。

### 8. 独立golden・敵対試験・完了条件

下表は手計算または独立calculatorで再現可能な仕様vector。formula専用の合成入力はmetric生成可能性を保証しないため、end-to-end fixtureと区別する。

| Vector | 入力と期待値 |
| --- | --- |
| G01 | K1 weights(1,3), losses(0,1) →3/4 |
| G02 | 同weights、losses(0,Unknown[0,1]) →[0,3/4] |
| G03 | K3 {a,b}/{b,c}, 全weight1 →2/3 |
| G04 | paired scores baseline(1,0),candidate(1,1) →1/2 |
| G05 | T: Dpar=1/4,Dsem=1,A=1/2 →1/4 |
| G06 | C_match: K0=80,κ=80,K=160 →1/2 |
| G07 | Q: K0=160,σ=80,K=80と240 →両方exp(-1)≈0.36787944117144232160 |
| G08 | Q: K∈[80,240],K0=160,σ=80 →[exp(-1),1] |
| G09 | uniform synthetic identity, A=C=1, archive includes exact baseline so N=1, Dctx=Dsem=L=0 →S1=1/6 |
| G10 | uniform identity, Preserve=1,Q=1,Rebind=Reframe=BreakCost=Redundancy=0、T=G=U=L=Rcounter=Bmem=0 →S2=5/32 |
| G11 | G10のRedundancy=1/4 →Gstruct=−1/20,S2=3/20 |
| G12 | raw δSはS(z)−実測S(z0)。identity候補のδS=0、rawは非zeroでもよい |
| G13 | loss interval[0.1,0.2],τ=0.2 →Pass；[0.2,0.3] →NeedsEvidence；[0.21,0.3] →Fail |
| G14 | c=3,κ_B=1 →Bmem=3/4；確認no path→1+Disconnected |
| G15 | Δcompression K0=80,K=40かつ保存合格→1/2；保存失敗ならIneligible |
| G16 | Brier baseline p=0,y=1、candidate p=1/2 →Δpredict=3/4 |
| G17 | K4 `[-2,3]×[-4,5]` →`[-12,15]`；除数区間が0を含む →Undefined/DivisionByZero |
| G18 | error e=5, epsilon=2, r=10（同一unit）→ell=3/10；1s→1000ms canonicalize後のidentity error=0 |
| G19 | UNSAT report without checked proof/enumeration →ValidationBasis=SolverReported、not independent validation |
| G20 | Q, K∈[200,240],K0=160,σ=80 →[exp(-1),exp(-1/2)] |
| G21 | Poincare κ=1, x=(0,0), y=(1/2,0) →d=acosh(5/3)=ln3≈1.098612288668109691 |
| G22 | No comparable registered RootGoalProfile →NoRecommendation; retain alternatives |
| G23 | Poincare κ=1 on one diameter: d(0,1/4)=ln(5/3), d(1/4,1/2)=ln(9/5), d(0,1/2)=ln3; the first two sum to the third. Node-ID relabeling leaves distances unchanged |
| G24 | Goal raw range [10,30], v=15 →max score 1/4, min score 3/4; v=31→InvalidInput |
| G25 | Two tied branches A<B by canonical key; relabel IDs leaves dispatch unchanged. Next budget epoch begins after persisted cursor even when frontier revision changed |
| G26 | CR-DESIGN accepted domain [-100,100], normalize [0,10], maximize x=12 →1 with SaturatedHigh; x=101→InvalidInput |
| G27 | B_mem: two paths with cost intervals [1,2]+[1,2] and [3,4] →minimum cost [2,4]; κ_B=2 gives B_mem=[1/2,2/3]. Open graph/no path observation →Unknown, not Disconnected |
| G28 | K4 family min{[0,10],[5,6]} enclosure →[0,6]; max →[5,10]; empty family min/max →Undefined/NoApplicable |
| G29 | Noncrossing Q with dmin/σ=2^-1000: outward exp bound may equal 1, but rational upper `1/(1+2^-1000)<1` retains strict noncrossing |
| G30 | Poincare κ=1, x=(0,0), y=(1/2,1/2), μ=1/4: q=sqrt(1/2); K8 refines internal precision above 64 bits until endpoint numerical slack<=2^-64 or reports ResourceExhausted |
| G31 | `d_near=[0,1]`, `s_geo=1`: `n_geo` encloses `[0,1−e^-1]` and remains ordered (`lower<=upper`) |
| G32 | Poincare κ=1, x=(1/2,0), y=(0,1/2): exact `t=8/9`, `q²=8/17`; K8 uses internal precision above 64 bits or returns a bounded ResourceExhausted result, never false convergence |
| G33 | Poincare κ=1, x∈[0,1/10], y=1/2: sound distance enclosure includes `[ln(27/11),ln3]`, width `ln(11/9)`; the large input interval is not a numeric-precision failure |
| G34 | `square([-1/10,1/10])` returns `[0,1/100]`; no negative norm-square lower bound is introduced |
| G35 | Frontier `{{a},{b}}`: for proposal `{a}` and `{a,b}`, union-distance values are `1/2,0`, nearest-verified-candidate distances are `0,1/2`; each is bound to a distinct `SchedulerNoveltyPolicyId` |
| G36 | New feature outside the fixed K3 universe remains `InvalidInput/UniverseMismatch`; a new ComparisonScope revision is required to compare it |
| G37 | Missing/unverified EmbeddingPolicy remains `Unsupported`; search proposal generation cannot create authority or exact semantic novelty |

全40契約にpositive、negative、選択/探索影響の3 test IDを付ける。REC等はscalar順位でなくledger→gate/queryへの影響を検査。少なくとも120契約test slotsを埋め、項目ごとに実行receiptを保持する。goldenは同じproduction関数を再呼出して生成しない。有限論理のn<=8は独立全割当oracle、K6は独立encoder、算術は別rational/high-precision実装と手計算を併用する。

追加必須：原式variable/sign mutant、全27係数、TL二重罰、unknownと0、N/A、空分母、無効σ、数値budget、source-sidecar漏洩、Γ矛盾、ASP brave/cautious、4値Both/Neither、時間反転/clock、pixel/world、oracle共通誤り、holdout leakage、2D→3D無校正、shared evidence、candidate mask改ざん、host未観測、復旧時の古い値、GPU誤差rank逆転を検査する。

仕様固定の出口：原案の式・全成分との対応、40契約の5欄、日英同内容、数値例、独立読者の敵対的点検を完了し、実装未完了を明記する。将来の実装完成の出口：全120 slots＋上記境界試験、leaf→formula→selection→production/cut/失効/再開の実観測、source/package/hostの各gateを別々に満たす。仕様書があるだけで実装を完成扱いしない。

### 8A. 固定registryと試験台帳

MetricId discriminantは下表へ固定する。P/N/Sはそれぞれpositive/negative/selection-impactの必須試験。対応本文の独立検証・選択影響を期待条件とし、slot存在を実行成功としない。

| MetricId | Tag (u16) | Positive | Negative | Selection / exploration |
| --- | --- | --- | --- | --- |
| TL-RECON | 0x0100 | TL-RECON-P | TL-RECON-N | TL-RECON-S |
| TL-SEM | 0x0101 | TL-SEM-P | TL-SEM-N | TL-SEM-S |
| TL-TIME | 0x0102 | TL-TIME-P | TL-TIME-N | TL-TIME-S |
| TL-SPACE | 0x0103 | TL-SPACE-P | TL-SPACE-N | TL-SPACE-S |
| TL-TOTAL | 0x0104 | TL-TOTAL-P | TL-TOTAL-N | TL-TOTAL-S |
| TL-PATH | 0x0105 | TL-PATH-P | TL-PATH-N | TL-PATH-S |
| TL-MODAL | 0x0106 | TL-MODAL-P | TL-MODAL-N | TL-MODAL-S |
| REC-D | 0x0107 | REC-D-P | REC-D-N | REC-D-S |
| REC-DELTA | 0x0108 | REC-DELTA-P | REC-DELTA-N | REC-DELTA-S |
| REC-U | 0x0109 | REC-U-P | REC-U-N | REC-U-S |
| REC-G | 0x010a | REC-G-P | REC-G-N | REC-G-S |
| CR-DCTX | 0x010b | CR-DCTX-P | CR-DCTX-N | CR-DCTX-S |
| CR-DPAR | 0x010c | CR-DPAR-P | CR-DPAR-N | CR-DPAR-S |
| CR-DSEM | 0x010d | CR-DSEM-P | CR-DSEM-N | CR-DSEM-S |
| CR-ACOMP | 0x010e | CR-ACOMP-P | CR-ACOMP-N | CR-ACOMP-S |
| CR-CMATCH | 0x010f | CR-CMATCH-P | CR-CMATCH-N | CR-CMATCH-S |
| CR-NKNOWN | 0x0110 | CR-NKNOWN-P | CR-NKNOWN-N | CR-NKNOWN-S |
| CR-TNC | 0x0111 | CR-TNC-P | CR-TNC-N | CR-TNC-S |
| CR-COVERAGE | 0x0112 | CR-COVERAGE-P | CR-COVERAGE-N | CR-COVERAGE-S |
| CR-COMPRESSION | 0x0113 | CR-COMPRESSION-P | CR-COMPRESSION-N | CR-COMPRESSION-S |
| CR-BRIDGE | 0x0114 | CR-BRIDGE-P | CR-BRIDGE-N | CR-BRIDGE-S |
| CR-PREDICT | 0x0115 | CR-PREDICT-P | CR-PREDICT-N | CR-PREDICT-S |
| CR-REDUNDANCY | 0x0116 | CR-REDUNDANCY-P | CR-REDUNDANCY-N | CR-REDUNDANCY-S |
| CR-GSTRUCT | 0x0117 | CR-GSTRUCT-P | CR-GSTRUCT-N | CR-GSTRUCT-S |
| CR-PRESERVE | 0x0118 | CR-PRESERVE-P | CR-PRESERVE-N | CR-PRESERVE-S |
| CR-REBIND | 0x0119 | CR-REBIND-P | CR-REBIND-N | CR-REBIND-S |
| CR-REFRAME | 0x011a | CR-REFRAME-P | CR-REFRAME-N | CR-REFRAME-S |
| CR-BREAK | 0x011b | CR-BREAK-P | CR-BREAK-N | CR-BREAK-S |
| CR-RGAIN | 0x011c | CR-RGAIN-P | CR-RGAIN-N | CR-RGAIN-S |
| CR-EXPLAIN | 0x011d | CR-EXPLAIN-P | CR-EXPLAIN-N | CR-EXPLAIN-S |
| CR-DESIGN | 0x011e | CR-DESIGN-P | CR-DESIGN-N | CR-DESIGN-S |
| CR-TRANSFER | 0x011f | CR-TRANSFER-P | CR-TRANSFER-N | CR-TRANSFER-S |
| CR-SOLVE | 0x0120 | CR-SOLVE-P | CR-SOLVE-N | CR-SOLVE-S |
| CR-UGAIN | 0x0121 | CR-UGAIN-P | CR-UGAIN-N | CR-UGAIN-S |
| CR-K | 0x0122 | CR-K-P | CR-K-N | CR-K-S |
| CR-QPARITY | 0x0123 | CR-QPARITY-P | CR-QPARITY-N | CR-QPARITY-S |
| CR-RCOUNTER | 0x0124 | CR-RCOUNTER-P | CR-RCOUNTER-N | CR-RCOUNTER-S |
| CR-BMEM | 0x0125 | CR-BMEM-P | CR-BMEM-N | CR-BMEM-S |
| CR-V1 | 0x0126 | CR-V1-P | CR-V1-N | CR-V1-S |
| CR-V2 | 0x0127 | CR-V2-P | CR-V2-N | CR-V2-S |

追加field：DefinitionRevision、ComparisonScopeId、MeasurementBindingDigest、GoalProfileId/GoalMetricId/RepresentativePolicyId、Source/Target/Context/BaselineRevision、ProbeSetDigest、LogicProfile、OutcomeKind、ComparatorId、TransformId、AcceptedInputRange、NormalizationMap、EncodingRevision、PolicyDigest、InputBasis、NumericPrecision、SemanticValidation、SemanticResult、ValidationBasis、OracleRevision、IndependenceStatus、SchedulerNoveltyPolicyId、SchedulerCanonicalKey/FairnessStateRevision/BudgetSeriesId/FairnessCursor、EvidenceRefs、DatasetRole、GeometryPolicyId/EmbeddingPolicyId、ResourceStatus、DependencyMetricIds。timestampやsource hashだけで独立性/真正性を推定しない。

### 9. 敵対的監査記録・残存不確定・停止／再開

2026-10-29の再監査で、式自体を変えずに以下の定義上の食い違いを修正した：

- source→target時間写像と損失誤差式の向きを統一し、canonical unit換算後に許容差epsilonと正の飽和尺度rを別々に適用。
- vector/space comparator、許容差、NormId、対象payloadをscope-bound Probe/MeasurementBindingへ固定。
- 区間の一般乗算・除算・abs・exp端点伝播、formula-DAG value identity、zero divisionを明示。
- solverのlogical resultと検証根拠を分離し、UNSAT report単体やsolver間一致をindependent validationへ昇格しない。
- RootGoalProfile/GoalMetricとscalarization・multiobjective代表規則を固定し、goal oracleなしの暗黙recommendationを禁止。
- OriginalScore区間が重なるときの未確定順位と、Pareto代表選択を分離。
- Poincare ball距離の領域・曲率・数値algorithmを追加し、座標生成・embedding objective・production consumer未登録はUnsupported/fallbackと明記。
- goal raw rangeの標準単調map、複数goalの明示policy、SchedulerCanonicalKey、frontier revisionを越えて継続するfairness cursorを定義し、opaque ID relabelで探索順が変わる穴を閉じた。
- CR-DESIGN等のsaturation boundsとraw AcceptedInputRangeを分離し、generic K7 clip禁止との定義衝突を解消。
- B_memを閉じたfinite graphのexact/interval shortest-path problemとして明記し、open registryでの「pathなし」をDisconnectedへ誤昇格しない規則を追加。

別実装の有理数/高精度十進計算で、区間演算、epsilon正規化、goal map、CR-DESIGN saturation、S2 baseline、片側Q interval、B_mem interval mapping、Poincare/atanh enclosureの代表vectorを照合した。これは数値例の独立再計算であり、形式証明・全成分実装・一般的意味妥当性の校正ではない。

残るhard design blockerはPoincare座標を作るEmbeddingPolicyと、そのproposalを実schedulerへ接続するproduction consumerである。距離式が固定されたことだけで、非ユークリッド探索が稼働・有効化されたとは扱わない。

有限probe設計、feature抽出の意味妥当性、oracleの独立性、重みの妥当性、一般化、自由言語/創作物の評価は引き続き校正・研究評価が必要。有限形式fragmentの厳密算術を普遍的意味距離と呼ばない。数学/論理・プログラム・自然言語・創作・その他行為という直観分類は課題設計の参考であり、直接5本の数値軸にしない。

### 9A. 添付 v1.1 数学再監査の照合・finite-formal-v1.4追補

照合した資料は`epistesys_tlcx_v1_1_mathematical_reaudit_2026-10-29.md`（SHA-256 `c4a6021dfd59303bec73f45f3d50712c33a603781e43f08526605e1c5d76f28b`）。同資料は`finite-formal-v1.1`を監査対象としているため、そのF-01〜03をそのまま現行欠陥とは扱わず、current v1.3本文と突き合わせた。

- **F-01**：current v1.3の`n_geo`式はすでに単調性に沿う`[1−upperExp(dL/s_geo),1−lowerExp(dU/s_geo)]`。修正は不要。v1.4では端点順の回帰golden G31を追加する。
- **F-02/F-03**：current v1.3は`p_inner=80`開始・32bit刻みの適応増加を持ち、endpoint計算の数値幅と`tL..tU`入力不確実性を別管理している。v1.1監査の同精度p=64固定／総距離幅への停止条件とは異なる。v1.4はirrational-q例G32と座標box例G33を加え、false convergenceを防ぐ回帰要件として保持する。
- **D-02**：汎用区間積で同じ不確実量を二乗すると不要な負下限を作り得る。v1.4ではK4に専用`square([a,b])`を追加し、K8のnorm-squareが座標差ごとのsquare→和になるよう固定する。正例G34を追加する。
- **D-01**：距離とfrontier-unionのK3は定義済みだが、nearest candidateからの距離とは別の目的量で、順位が逆転し得る。v1.4では`DistanceToFrontierUnion-v1`と`NearestVerifiedCandidate-v1`を別`SchedulerNoveltyPolicyId`にする。現行union参照挙動は維持し、根拠なくnearestをdefaultへ置換しない。同一scope・等budgetの比較G35と後続校正を要求する。
- **B-01/B-02**：固定K3 universe外のfeatureは依然`UniverseMismatch`であり、新しいComparisonScope/revisionを要する。EmbeddingPolicyとproduction consumer未接続の境界も維持し、Unsupported/fallbackを成功へ昇格しない（G36/G37）。

`finite-formal-v1.4`はこの仕様追補のみを表し、製品version、F1/F2原式、測定実装、校正、性能または意味的有効性を更新・実証したものではない。

Assurance-CompilerはTLCXの選択Rust単位への補助監査に限定され、`needs_evidence`/`blocked_with_diagnostics`/未監査範囲を保持。本Markdownと数式の証明・校正・repository-wide safetyを認証したものではない。RepoSeiriも文書・参照整合の補助であり数学oracleでない。

本仕様固定後、くすりで完了通知を一度送信し、配信結果を別のprivate receiptへ保存する。その後本セッションの全作業を停止する。再開指示時は[進捗・引継ぎ](tlcx-implementation-progress-2026-10-28.md)から既存差分、TLCX-01の出口未確定、TLCX-02下書き、host G13未観測を再確認する。自動再開・commit/push・version更新・plugin更新はこの仕様固定では行わない。

---

# TL and S_Creative measurement contract v1 — fixed pre-implementation specification

## English

### 0. Status, original-formula fidelity and precedence

- Fixed: 2026-10-29 JST (original reference and TLCX design fixed on 2026-10-28). Preserve the original formula revisions; operational conventions advance to `finite-formal-v1.4` (v1.1–v1.3 hashes remain in the append-only chain). The attached v1.1 mathematical re-audit is reconciled against current v1.3 rather than treated as a command or as current-version evidence. Source implementation, calibration and TLCX resumption are outside this specification revision.
- This is a normative specification of definitions, algorithms, applicability, independent validation and selection effects, not an implementation, calibration or general semantic/creativity-performance claim.
- Fixed original-concept SHA-256: `1bd7540cbb53a907ecd84c1fe0ebbd6bcaf5d495bcf69f75170bf155db386d04`. Do not rewrite or reproduce the source document in this repository.
- `OriginalFormula/F1-v1` and `OriginalFormula/F2-v1` retain the formulas, signs and terms below. The original did not define every calculation. Finite-set, distance and cost recipes here are explicitly added `OperationalMeasurement/finite-formal-v1.4` conventions, not retrospectively attributed to the original.
- v1 D_ctx and v2 D_par are separate metrics, as are C_match and Q_parity. No implicit aliases. Original Q_parity penalizes equal simplification and complexification equally; asymmetry requires a different FormulaRevision.
- Separate record U (Uncertainty) from U_gain (Utility Gain), and creative D_sem (configuration difference) from TL semantic loss (preservation defects), by type and scope.
- TLCX slices 17–25, 27–29 and 38–41 in the [roadmap](epistesys-tlcx-contextual-multilogic-loss-creativity-roadmap-2026-10-28.md) depend on this contract. Implementers must not invent or silently change measurement conventions.

### 1. Authoritative formulas


### Formula F1 — original S_Creative v1
\[
S_1(z)=\alpha D_{\mathrm{ctx}}(z)+\beta D_{\mathrm{sem}}(z)
-\lambda L_{\mathrm{trans}}(z)+\gamma A_{\mathrm{compat}}(z)
+\delta C_{\mathrm{match}}(z)-\eta N_{\mathrm{known}}(z).
\]
### Formula F2 — original S_Creative v2
\[
S_2(z)=w_1T_{\mathrm{nc}}(z)+w_2G_{\mathrm{struct}}(z)
+w_3R_{\mathrm{gain}}(z)+w_4U_{\mathrm{gain}}(z)
+w_5Q_{\mathrm{parity}}(z)-w_6L_{\mathrm{trans}}(z)
-w_7R_{\mathrm{counter}}(z)-w_8B_{\mathrm{mem}}(z).
\]
\[
T_{\mathrm{nc}}(z)=\sqrt{D_{\mathrm{par}}(z)D_{\mathrm{sem}}(z)}
\,A_{\mathrm{compat}}(z).
\]
\[
G_{\mathrm{struct}}=
a_1\Delta coverage+a_2\Delta compression+a_3\Delta bridge
+a_4\Delta predict-a_5Redundancy.
\]
\[
R_{\mathrm{gain}}=
b_1Preserve+b_2Rebind+b_3Reframe-b_4BreakCost.
\]
\[
U_{\mathrm{gain}}=
c_1\Delta explain+c_2\Delta design+c_3\Delta transfer+c_4\Delta solve.
\]
\[
Q_{\mathrm{parity}}(z)=
\exp\!\left(-\frac{|K(z)-K_0|}{\sigma}\right).
\]

### 2. Required inputs, states and semantic boundaries

| Input | Required material |
| --- | --- |
| ComparisonScope | Root question; original context revision; allowed reconstructed-context family; hard invariants; mutable premises; baseline; GoalProfileRevision/goal policy; units; pinned probe/feature/bridge/corpus sets and digests |
| Candidate | Source/context; typed logical form; explicit Delta; models/derivations/artifacts; correspondence among inputs, queries, results and reconstructions |
| MeasurementBinding | ComparisonScopeId, CandidateId, candidate-specific source/target/context revisions, producer/transform/result digests, actual payload/evidence references |
| Probe | Unique ID; logic/fragment; root-source property/query; target contract/expected relation; OutcomeKind; ComparatorId; source→target TransformId; unit/dimension; epsilon/range scale; raw AcceptedInputRange and NormalizationMap when applicable; positive rational weight; OracleRevision; independent-check scope; required input basis |
| MetricPolicy | MetricId/DefinitionRevision; RootGoalProfileId; active components; normalization; weights/scales; applicability mask; numeric budget; gates/selection mode; pin SchedulerNoveltyPolicyId when geometric scheduling is enabled |
| Observation | Pins for query/context/solver/version/options/domain/result/countermodel/proof; acquisition state; independent checking; scope; dataset role |

`OutcomeKind` is one of `BooleanRelation/FiniteRelation/ScalarQuantity/VectorQuantity/RegisteredSemanticOutcome`. For Boolean/finite relations, compare whether the relation holds after the allowed source→target map; loss is 0 if preserved and 1 if violated. For numeric quantities, a registered ComparatorId maps to identical types/units and compares the mapped source value with the target. Scalars use `e=|v_target−F(v_source)|`; vectors use a NormId fixed by ComparatorId. Continuous loss is `ell=min(1,max(0,e−epsilon_q)/r_q)`, where `epsilon_q>=0` is the allowed discrepancy and `r_q>0` the saturation scale, both in the same unit as e. Pin epsilon after scope construction. If e is an interval, evaluate the monotone function with outward bounds. Time/coordinate errors use the same rule. Unregistered comparator/unit/norm/map is Unsupported/NoScalar; implementers must not invent a default distance or tolerance.

Raw quantity `AcceptedInputRange` is distinct from normalization bounds/maps. A saturating map is allowed only when registered for that MetricId; retain the raw value, low/high saturation flag, and map revision. Values outside the accepted domain are InvalidInput; do not turn arbitrary raw outliers into successful bounded scores.

Duplicate IDs are InvalidInput. Reject hard-invariant deletion or original-context overwriting before scoring. Premise reconstruction is a primary objective, but authority is not a search variable.
Distinguish Exact, Bounded, Estimated, Unknown, NotApplicable, Incomparable, Undefined, InvalidConfig and ResourceExhausted. Numeric payloads are only `MeasurementValue::Exact{q}`, `Bounded{lo,hi}` and `Estimated{lo,hi,method}`; other states carry typed reasons, not numeric zero. Store `InputBasis` (Given/Assumed/Observed), `SemanticValidation` and `IndependenceStatus` as orthogonal fields. Exact means arithmetic precision for declared inputs only, not world truth, correct parsing or proof.
Only embed Unknown as [0,1] when the quantity/range is already defined in scope and observations alone are missing. Do not collapse Incomparable/Undefined/InvalidConfig to Unknown. ResourceExhausted may retain a valid partial bound but is not completion. Empty denominators are generally Undefined/NoApplicable, not zero, except explicit piecewise-zero definitions for K3 valid empty-set pairs and CR-REDUNDANCY confirmed empty representations. Confirmed empty N_known corpora give max∅ and Undefined/EmptyCorpus; unread/unavailable corpora are Unknown. Neither implies zero.
N/A requires applicability justification at scope construction. Candidate-driven information loss cannot become N/A. Strict-original formulas require every referenced component to have a valid number/defined bound, otherwise NoScalar. No implicit N/A-to-zero, candidate-specific reweighting or rescue through 0×Undefined. Zero coefficients are not measured zeros.
CandidateEligibility is a separate Eligible/Ineligible/Pending field, not a numeric measurement state. Unchecked prerequisites yield PrerequisitePending/NoScalar; confirmed prerequisite failure yields Undefined/PrerequisiteFailed and candidate Ineligible.
Use identical ComparisonScope and evaluation policy across candidates. Candidate-specific source/target/context revisions belong in MeasurementBinding, not the shared-scope hash required equal across candidates. Scope fixes original context, allowed context family and root-goal mapping policy; validate candidate membership/map-back. Baseline remains common. Recommendations must state contexts, premise changes, uncertainty and optimality scope.
RootGoalProfile binds at least one fixed GoalMetricId with raw unit/type/range/direction, score map, oracle revision, budget and evidence policy. For scalar raw range [m,M], require M>m; the standard map is (v−m)/(M−m) for maximize and (M−v)/(M−m) for minimize. Apply monotone maps endpointwise to intervals. An Exact raw value outside the declared range is InvalidInput/OutOfRange. A bounded interval crossing a range boundary is NoScalar/NeedsEvidence; an interval wholly outside is InvalidInput. Never clip. Nonlinear maps and categorical score tables must be fully recorded in the ProfileRevision. Multiple objectives remain separate Pareto dimensions. Scalarization weights or lexicographic representative order belong to an explicit ProfileRevision and cannot be added later. Without a GoalProfile or comparable goal observation, do not issue a goal-based recommendation.

### 3. Shared computational kernels

**K1 Fixed-set weighted assessment:** pre-fix nonempty P, wi>0 and W=Σwi>0. For loss intervals [li,ui]⊆[0,1], return [Σwi li/W,Σwi ui/W] using rational arithmetic. Allowed preservation is 0, confirmed improper alteration/drop is 1, missing observation is [0,1]. Continuous distances follow each metric's declared rule. Denominator and applicability mask are candidate-independent.
**K2 Paired gain:** use the same P/oracle/budget/unit, first form gi=score_z(i)−score_0(i), then Σwi gi/W. Scores in [0,1] yield gains in [-1,1]. Never subtract rounded means. Uncertain inputs produce interval differences; scope mismatch is Incomparable. Self-subtraction of the same valid observation node preserves dependence and gives Exact 0 without erasing input uncertainty/basis; distinct nodes with equal intervals use ordinary interval differences.
**K3 Weighted set distance:** fix universe U and vi>0. Weighted Jaccard distance is Σ_(A△B)vi/Σ_(A∪B)vi. Return zero for two confirmed valid empty sets only. Unextracted sets are not empty. Identity is logic-specific canonical meaning, not raw IDs/hashes/order. Check A∪B⊆U; features outside U are InvalidInput/UniverseMismatch, never silently dropped.
**K4 Interval arithmetic:** for rational intervals A=[a,b], B=[c,d], addition is [a+c,b+d], subtraction [a−d,b−c], multiplication [min(ac,ad,bc,bd),max(ac,ad,bc,bd)]. For the same uncertain scalar squared, use the dedicated `square([a,b])`: [0,max(a²,b²)] when a<=0<=b, otherwise [min(a²,b²),max(a²,b²)]. Build Poincare norm-squares by squaring each coordinate-difference interval and summing; do not use generic interval multiplication X×X for a dependent square. For a finite nonempty family Xi=[li,ui], min enclosure is [min li,min ui]; max enclosure is [max li,max ui]. Empty min/max is Undefined/NoApplicable. Negation is [−b,−a]. Absolute value is [0,max(−a,b)] if a<=0<=b, otherwise [min(|a|,|b|),max(|a|,|b|)]. Sqrt requires a nonnegative lower endpoint and uses rational outward bounds. Division by an interval containing zero is Undefined/DivisionByZero; otherwise multiply by its reciprocal. `exp(−[x,y])` for 0<=x<=y is [lowerExp(y),upperExp(x)]. Round outward. Formula-DAG ValueNode identity includes MetricId, DefinitionRevision, MeasurementBindingDigest and ProbeSetDigest. A shared node is evaluated once; only self-subtraction of that node is dependence-preserving Exact 0. Equal-width intervals on distinct nodes use ordinary interval arithmetic. Enclosures that ignore correlation are conservative, not independent error probabilities.
**K5 Solver truth bridge:** first check classical Γ consistency. Γ UNSAT gives ContextInconsistent and stops classical scoring. If Γ SAT, UNSAT of Γ∧¬φ entails support for φ by the declared semantics; UNSAT of Γ∧φ supports ¬φ; both SAT means undetermined. This is the logical relation implied by a solver report, not independent validation of that report. Separate `SemanticResult` (ContextInconsistent/Supported/Refuted/Undetermined/Unknown/Conflict) from `ValidationBasis` (SolverReported/ModelRechecked/ProofChecked/FiniteExhaustive). A SAT witness is ModelRechecked only after an independent evaluator checks the complete model against the exact formula. UNSAT is ProofChecked/FiniteExhaustive only after a registered proof checker validates its certificate or independent exhaustive enumeration covers the declared finite domain. Otherwise retain SolverReported and do not count it as independent validation. Never guess after timeout/unknown. Agreement among multiple solvers is not independent corroboration until shared parser/translator dependencies are checked. Pin ASP stable-model/brave/cautious and explicit/default negation semantics; pin finite/infinite temporal traces and quantifiers. Never map four-valued Both/Neither to classical False. SAT is neither entailment nor optimality.
**K6 Canonical description length:** K=8×len(E(term)) bits for a finite prefix-free typed-term encoding. Records use tag:u8 plus payload. Bool uses canonical u8 0/1; Integer uses sign:u8 plus shortest ULEB128 magnitude-byte length plus little-endian magnitude (zero positive, one zero byte); Symbol uses ULEB128 UTF-8 length plus bytes; Apply uses operator Symbol, ULEB128 arity and ordered child encodings; Binder uses a fixed binder tag, sort Symbol and body with de Bruijn indices. Fix tags, shortest ULEB and grammar revisions. Exclude incidental IDs, receipts, caches, comments and formatting. Never sort semantically ordered children. Beyond alpha-renaming, canonicalization requires logic-checked equivalences. Unsupported graph-to-term lowering is Unknown. K is a proxy, not Kolmogorov complexity; K0>0 and σ/κ use bits.
**K7 Numeric precision:** use bounded BigRational with signed numerator, positive denominator and GCD normalization. Reference caps: numerator/denominator 65,536 bits each; probes/features 100,000 each; corpus 10,000 items; bridge graph 100,000 edges. Exceeding caps gives ResourceExhausted. Fix integer precision 1<=p<=256 in the Profile (reference default 64). Bound sqrt on a p-bit grid using integer square comparisons as [q/2^p,(q+1)/2^p]; return a rational Exact value only when both rational numerator and denominator are perfect squares. K7 runs after the Metric-specific NormalizationMap. An already-normalized leaf outside [0,1] is InvalidInput, never automatically clipped. Raw-input saturation is allowed only for components with an explicit Map (e.g. CR-DESIGN), retaining AcceptedInputRange, raw value and SaturatedLow/High status.
For exp(−x), x>=0, x=0 gives Exact 1. Find the smallest r>=0 satisfying x/2^r<=1/2 by rational comparisons (no floating log); set y=x/2^r. Let S_m=Σ_(k=0)^m(-y)^k/k!; S_(2j+1) is a lower bound and S_(2j) an upper bound. Update terms exactly by term×y/k and stop when gap<=2^{−(p+r+16)}. Convert bounds to a P=p+r+16-bit grid outward: lo=floor(L*2^P)/2^P, hi=ceil(U*2^P)/2^P. For each of r nonnegative interval squarings use lo'=floor(lo²*2^P)/2^P and hi'=ceil(hi²*2^P)/2^P. Require final width<=2^-p; otherwise increase P by 32 and recompute up to three times from the unrounded rational Taylor bounds. Reference p=64, r<=64, terms<=256; reject r>64 before shifts/Taylor work. Budget exhaustion retains valid bounds and ResourceExhausted; underflow is not Exact 0. For K interval [kL,kU], dmin=0 if kL<=K0<=kU, else min(|kL−K0|,|kU−K0|); dmax=max(|kL−K0|,|kU−K0|). The Q interval is [lowerExp(dmax/σ),upperExp(dmin/σ)]; the exact supremum is 1 if the closed interval contains K0 (but enclosure does not prove an attainable candidate); finite-precision outward upper may still equal 1 without crossing unless tightened. For dmin>0 and x=dmin/σ use `upper_Q=min(upperExp(x),1/(1+x))<1` (from e^x>1+x). An upper of 1 for an enclosure containing K0 is only an outer bound, not Exact 1. Do not use endpoint-only maxima. A [0,u] lower endpoint does not imply Exact 0.

IndependenceStatus distinguishes Unassessed, SharedDependency, ReportedSeparation and CheckedSeparation {checked_layers,gaps,evidence_refs}. Registry entries or separate runs alone never upgrade to CheckedSeparation. Its constructor checks scope-bound separate checker/lowering, gold provenance, dataset splits and shared-dependency ledgers, recording only actually checked layers. Caller text/self-attestation stays ReportedSeparation. Authenticity separately checks observation receipts against configured trust roots; do not infer key-custody or human independence. Retain unverified layers as gaps.

### 4. Parameters, profiles and uncalibrated preferences

There are 27 original coefficients: six F1, eight F2, five a, four b and four c. σ is a separate scale. TL-axis weights, κ_M/κ_B, feature/probe weights and gate thresholds also enter ProfileDigest.
Coefficients are finite nonnegative rationals, an added operational restriction. The original does not require sums of one. Derive ranges from actual coefficients without silent normalization. All-zero coefficients algebraically give zero but are DegenerateWeights and rejected for operational ranking. Require positive σ/κ_M/κ_B/K0; missing means MissingParameter and invalid means InvalidConfig. Never rescue with epsilon or one.
`reference-uniform-test-v1` uses F1 coefficients 1/6, F2 1/8, a 1/5, b/c 1/4, σ=K0, κ_M=K0 and fixture-defined κ_B. These are explicit golden-test preferences, not calibrated production defaults.
Determine TL applicability and weights once from root context, normalize over those axes once and retain the same denominator for all candidates. All active weights zero is Undefined. No per-candidate dropping/reweighting.
`OriginalFormula/F1-v1` and F2-v1 are strict scores; `TLCX/JointPareto-v1` is a separate decision profile. Display original formulas separately from modifications. Label uncalibrated parameters DeclaredPreference/Uncalibrated; exact arithmetic is not research validity.

**K7 detail:** for x=n/d>=0 use q=isqrt(floor(n·2^(2p)/d)); verify bounds by squaring. Perfect-square numerator and denominator permit Exact rational results. Negative square roots, nonfinite inputs and normalized [0,1] leaves outside their domain are InvalidInput, not clipped. Raw-value saturation is permitted only by a Metric-specific Map with a separately declared AcceptedInputRange.

### 3A. Kernel details implementers must not invent

- K6 tags: 0x01 Bool, 0x02 Integer, 0x03 Symbol, 0x04 Apply, 0x05 Binder, 0x06 BoundVar. BoundVar carries the shortest ULEB128 index. Binder carries kind:u8 (0 forall, 1 exists, 2 lambda), E(Symbol sort), then E(body). Apply operators also use E(Symbol). Other tags are Unsupported. Bound indices must be below current binder depth. Symbols compare exact UTF-8 bytes without implicit Unicode normalization; other canonicalizations require another profile.
- Validate lengths/arity, caps, remaining bytes and depth before reading children. Maximum depth is 256 and term size 16MiB. Integer signs are 0=positive or 1=negative only; others are InvalidInput. Magnitude length must be >=1. Reject cycles, leading zero magnitude bytes only when len>1, negative zero, overlong ULEB, Bool 2, unbound indices and trailing bytes. Bundle is Apply(Symbol `bundle-v1`, [semantic_term, decoder_term, dictionary_term]) with exactly three children; absent decoders/dictionaries are explicit Bool(false).
- K6 term goldens: Bool(false)=`01 00` (16 bits); Symbol("a")=`03 01 61` (24 bits); Integer(0)=`02 00 01 00` (32 bits); Apply("f",[Bool(false)])=`04 03 01 66 01 01 00` (56 bits). These are term vectors, not full Bundle lengths.
- Sorted canonical-set merge makes K3 O(|A|+|B|); K1/K2 are O(n); exhaustive archive comparison is O(corpus×features); DAG evaluation O(nodes+edges); Dijkstra O((V+E)log V). Budget all inputs and intermediate big integers; use checked count/length arithmetic.
- Reference formal fragments: exhaustive propositional assignments with n<=16; finite typed relations with <=100,000 total domain tuples; explicit finite traces of length<=256; finite four-valued queries; ground ASP with model cap 256. SMT/ASP support only registered fragments. Exceeded caps, unsupported logics or unproved infinite-domain claims remain Unsupported/NeedsEvidence, never formal validation through approximation. Extensions require new fragment revisions.
- Reject DAG cycles, unknown MetricIds, duplicate definitions and scope mismatch. Original-formula repeated references with different coefficients are allowed, but shared-input dependencies remain visible. Use enums/typed IDs and a fixed registry internally, not string-name comparisons.
- Restrict calculation profiles to BinaryOracle or NormalizedQuantity {unit,m,M,direction}. Free-rubric judgments are separately pinned Estimated profiles. Implementers must not invent missing oracles, extractors or bridge correspondences.

### 5. Reading the component catalogue

The following 40 contracts cover TL auxiliaries, four records, every original-formula component and its children. Each has the five user-required fields; common inputs, K1–K7 and failure rules also apply. Independent-validation entries specify future tests, not a claim that this document ran them.

#### TL-RECON — reconstruction loss

- **Mathematical definition, range and units**：Fraction of protected source features unreconstructable by an allowed inverse H(Y). Apply K1 to fixed P_recon. L_recon is dimensionless in [0,1], distinct from byte similarity.
- **Calculation algorithm**：Give only target semantic payload to an isolated decoder; reconstruct; compare using fixed feature oracles; map failure/success/unobserved to 1/0/[0,1]; apply K1. Pin decoder, dictionary, model and side information, also accounting for them in compression cost.
- **Applicability**：Finite tasks with declared reconstructed objects, equivalence and decoder. Source lookup through sidecars or IDs is forbidden. A target genuinely containing the full source can score zero, but this proves neither understanding nor compression.
- **Independent validation**：Use an independent decoder, exhaustive small inputs, hand-calculated single-feature deletion and source-access isolation. Test identity zero, protected-feature deletion positive and ID-renaming invariance.
- **Effect on selection**：An increase raises active aggregate loss. Required reconstruction defects separately fail a hard gate. Direct failed probes to re-lowering or alternative representations; zero never issues Validated.

#### TL-SEM — semantic preservation loss

- **Mathematical definition, range and units**：Weighted fraction violating required semantic-preservation probes, each explicitly declaring equivalence, soundness or completeness. K1 gives a dimensionless [0,1] loss, not creative D_sem.
- **Calculation algorithm**：Lower source/target queries using a fixed mapping and check each context's consistency first. For a satisfiable Γ, mark φ Supported only when Γ∧¬φ is UNSAT; Refuted only when Γ∧φ is UNSAT; both SAT means Undetermined. Equivalence requires both directional entailment queries Supported; soundness requires every target-claimed φ_t's reverse map be Supported by the source; completeness requires every Required source claim φ_s map to a target-supported claim. An UNSAT context is ContextInconsistent. Conflict/Unknown/timeout/Unsupported does not fix a loss; retain its typed reason. Score 1 only for independently confirmed violation and 0 only for complete pass.
- **Applicability**：Checkable fragments with pinned logic signatures, quantifiers, domains and semantic correspondence. Cross-logic comparisons require checked bridges. General understanding of arbitrary prose is outside scope.
- **Independent validation**：Use small-model enumeration without the shared parser, independent type/effect checks and manual gold mutations of negation, quantifiers, scope and exceptions. Context inconsistency, Both/Neither and timeouts are not success.
- **Effect on selection**：Return preservation defects to TL and their obligations. Novelty cannot compensate for violated hard claims. Record permitted novelty as Delta rather than a TL defect unless required preservation fails.

#### TL-TIME — temporal preservation loss

- **Mathematical definition, range and units**：Violations of fixed temporal-order, duration or absolute-time probes. Binary relations score 0/1. Numeric probes compute `e_q=|v_target−F_q(v_source)|` in a common canonical unit, then `ell=min(1,max(0,e_q−epsilon_q)/r_q)`. r_q>0 and allowed discrepancy epsilon_q>=0 share e_q's unit; K1 aggregates to dimensionless [0,1].
- **Calculation algorithm**：Validate ClockId, TimeScale, Origin, Unit, Precision and ReferenceId; first convert exactly to common canonical units. F maps canonical source time to canonical target time. Absolute instants use `F_abs(t)=a*t+b`; elapsed durations use `F_dur(Δt)=a*Δt`, without b. Intervals are nonempty `[start,end)` with start<end; reversed/empty intervals are InvalidInput and instantaneous events are a separate type; before means endA<startB and meets means equality. Order, duration and absolute timestamps are separate probes. Apply epsilon/r_q to error bounds with outward rounding.
- **Applicability**：Requires calibration and clock correspondence. After canonical unit conversion, residual `t'=a*t+b` preserves order when a>0. Preserving duration requires residual a=1; absolute-time preservation requires residual a=1 and b=0, or an explicitly calibrated origin map. A reversal from a<0 is a preservation defect unless declared as a separately permitted Delta. Permitted Delta cannot delete hard temporal obligations. Temporal order is not causality.
- **Independent validation**：Use an independent rational-time checker, unit-invariance, before/meets boundaries, reversal, mismatched clocks and missing-as-zero mutations. Propagate precision/tolerance into bounds.
- **Effect on selection**：Return active temporal loss and affected obligations for repair. Uncalibrated clocks remain Unknown/Incomparable; spatial or creative gains cannot fill missing time evidence.

#### TL-SPACE — spatial preservation loss

- **Mathematical definition, range and units**：Violations of fixed spatial topology, containment, direction or metric probes. Relations score 0/1. Numeric probes use a ComparatorId-fixed `e_q=ComparatorId(F(source),target)` and `ell=min(1,max(0,e_q−epsilon_q)/r_q)`. epsilon_q>=0 and r_q>0 share e_q's unit; K1 yields dimensionless [0,1].
- **Calculation algorithm**：Validate FrameId, dimension, axes, origin, unit, handedness, object correspondence and TransformClass. After allowed mapping, use registered ComparatorIds for distances/angles/containment errors. Pin vector NormId, angular wrap rule, epsilon and r_q to the policy revision; missing comparators are Unsupported. Propagate transform/measurement intervals through to loss.
- **Applicability**：Preservation differs across rigid, similarity and affine maps. Do not compare pixels with world coordinates or 2D with 3D without calibration. Permitted reflections are not inherently errors. This is distinct from search geometry.
- **Independent validation**：Use an independent geometry checker, hand-calculated small shapes, rigid/unit invariance, similarity without distance preservation, object relabeling and missing-depth adversarial vectors. Propagate transform uncertainty.
- **Effect on selection**：Return active spatial loss and specific defects. Uncalibrated inputs stay unknown; proximity in exploration geometry never establishes spatial preservation.

#### TL-TOTAL — L_trans

- **Mathematical definition, range and units**：L_trans=Σ_(a∈A)ρ_a L_a/Σρ_a with positive weights and pre-fixed applicable axes A. Dimensionless [0,1]. The original left aggregation unspecified; this convention completes it.
- **Calculation algorithm**：Fix mask/weights from the root context; validate axis results; aggregate intervals by rational weighted sums. W=0 is Undefined. Exclude N/A axes only when constructing the scope, never per candidate.
- **Applicability**：Requires comparable normalization, targets and preservation obligations within one scope. F1/F2 bind this L_trans to an explicit TLReductionPolicy ID.
- **Independent validation**：Check with independent rational arithmetic: axis-order invariance, all-zero/all-one losses, proportional weights, one unknown axis, empty denominator and rejection of candidate-driven masks.
- **Effect on selection**：Apply −λL in F1 or −w6L in F2 once. Check hard-axis thresholds separately so averaging cannot hide a critical defect. JointPareto retains loss as a separate dimension.

#### TL-PATH — end-to-end preservation

- **Mathematical definition, range and units**：L_end in [0,1] directly rechecks root-source protected probes at the final target. It is not a sum of edge losses. D-ledger defects use unique (root obligation, property, revision) keys.
- **Calculation algorithm**：Store per-edge mappings/witnesses; check composition; remeasure final-target TL axes; coalesce the same root defect rather than repeatedly charging it.
- **Applicability**：Requires composable correspondence. Record final preservation separately from intermediate loss when information is later reacquired. Never infer restoration without evidence.
- **Independent validation**：Exhaust small paths; test identity-edge insertion, repeated-defect invariance, drop/restore with and without evidence, and unequal forward/reverse TL.
- **Effect on selection**：Final gates use L_end plus individual hard obligations, not merely small local or summed losses. Path history guides where to re-explore.

#### TL-MODAL — modal_likeness

- **Mathematical definition, range and units**：Weighted fraction of fixed required capabilities confirmed supported; success-form K1, dimensionless [0,1]. This auxiliary suitability is not a truth probability.
- **Calculation algorithm**：Require both registry declarations and revision-bound execution probes. Declaration alone is unobserved. Average success 1, confirmed unsupported 0 and unknown [0,1] over a fixed denominator.
- **Applicability**：Comparable representation/backend capabilities for the same task. Keep unsupported distinct from unobserved.
- **Independent validation**：Test false registries, swapped backends, cut probes and independent fixed-denominator calculations. Slot presence alone must never yield 1.
- **Effect on selection**：Use for representation proposals and exploration budgets, never as TL preservation, permission or output-commit evidence.

#### REC-D — four-record Defect

- **Mathematical definition, range and units**：D is a finite defect-event set, not a scalar. Keys identify root obligation, property and source/target revisions; each event links TL axes and evidence. Event count is not TL.
- **Calculation algorithm**：Create only from confirmed preservation violations; deduplicate by root key; link spans, transforms and observations. Unobserved cases belong in REC-U, not confirmed D.
- **Applicability**：Requires prior separation of permitted Delta from preservation duties. Delta cannot justify deleting a hard obligation.
- **Independent validation**：Independently compare hand-built ledgers containing one defect, duplicate paths, permitted reconstructions and unobserved cases.
- **Effect on selection**：Connect to obligation repair and hard gates. Gains never erase defect facts; raw counts do not rank candidates.

#### REC-DELTA — four-record permitted change

- **Mathematical definition, range and units**：Delta is a typed edit set carrying original/candidate contexts, targets, before/after, reason, permission basis and map-back. Its count is not distance or gain.
- **Calculation algorithm**：Extract edits to mutable premises/frameworks; reject intersection with hard duties; record logic-specific meaning; feed Dctx/Dpar/Dsem. Keep the original context immutable.
- **Applicability**：Supports premise reconstruction as a primary objective. Changing the root question or hard invariants requires a new ComparisonScope. Execution authority is never reconstructed.
- **Independent validation**：Use independent diffs, exhaustive small contexts, ID-only changes, deleted hard duties and concealed domain changes.
- **Effect on selection**：Justify comparison of reconstructed branches without presenting an answer in a new context as proof in the original one.

#### REC-U — four-record uncertainty

- **Mathematical definition, range and units**：U is a set of unobserved, ambiguous, incomparable, unsupported or unverified gaps, not probability or U_gain. Report counts/scopes without collapsing them to confidence.
- **Calculation algorithm**：Record unresolved metric/probe states and reasons by unique keys, retaining required evidence and next actions. Never collapse all states to false.
- **Applicability**：Required for every transform. Distinguish undefined quantities from merely missing observations.
- **Independent validation**：Test timeout, inapplicability, zero denominators, inconsistency and unsupported inputs for distinct states and absence of success receipts.
- **Effect on selection**：Drive Clarify, Hold, added probes or exploration allocation. Reducing U alone establishes neither semantic gain nor correctness.

#### REC-G — four-record gain

- **Mathematical definition, range and units**：G is an event set for validated preservation, reorganization or capability change relative to a fixed baseline. Events reference metrics, primitive features and paired observations; it is not G_struct itself.
- **Calculation algorithm**：Tag observed and predicted gains separately; detect duplicates through shared primitive IDs; aggregate into creative components. Removing hard conditions is not gain.
- **Applicability**：Requires correspondence to the same root goal, baseline and fixed evaluation set. Easier tasks are not comparable gains without a common mapping.
- **Independent validation**：Independently test paired examples, renamed duplicate bridges and predictions falsely promoted to observations.
- **Effect on selection**：Supply gain evidence and alternative explanations, exposing shared dependencies instead of counting them as independent votes.

#### CR-DCTX — D_ctx

- **Mathematical definition, range and units**：K3 distance between mutable premise/fact feature sets of original and reconstructed contexts; dimensionless [0,1]. Axiom/inference-rule signatures belong separately to D_par.
- **Calculation algorithm**：Canonicalize both sets in the pinned universe; compute weighted symmetric difference over union. Changes to immutable features fail a hard gate first.
- **Applicability**：Contexts admitting typed mutable-premise representations. Do not silently include framework changes.
- **Independent validation**：Independent set arithmetic, ID/order invariance, distance 2/3 for {a,b} versus {b,c}, and missing-versus-empty tests.
- **Effect on selection**：Contributes +α in F1. With positive α and satisfied gates, larger distance raises raw score, but other terms or gates may prevent adoption. Novelty is not correctness.

#### CR-DPAR — D_par

- **Mathematical definition, range and units**：K3 distance between framework signatures: logic, axioms, ontology sorts, inference rules and operator meanings. Dimensionless [0,1], separate from Dctx.
- **Calculation algorithm**：Declare signature correspondence and apply K3. Identically named operators with different meanings are distinct features; mere renaming is invariant.
- **Applicability**：Framework reconstructions with bridges to the same root goal. For integers versus F2, declare numeral labels and element mappings; answers in distinct systems are not interchangeable.
- **Independent validation**：Independent exhaustive small signatures, operator-meaning changes, sort changes and rename-only cases.
- **Effect on selection**：Feeds Tnc in F2. Dpar=0 forces Tnc=0, not all other gains or the entire S2.

#### CR-DSEM — D_sem

- **Mathematical definition, range and units**：Weighted fraction of fixed nonmandatory meaning-configuration queries whose canonical outcomes differ between baseline and candidate, dimensionless [0,1]. Required preservation belongs to TL-SEM.
- **Calculation algorithm**：Evaluate queries in one scope; compare typed relations/assignments; apply K1 to different 1, same 0 and unknown [0,1]. Cross-logic outcomes require mappings.
- **Applicability**：Requires a fixed query universe, outcome equivalence and mutable aspects. Distance is neither automatically loss nor usefulness.
- **Independent validation**：Independent small models, synonymous relabeling at zero, valid reconfiguration above zero and required-relation drops also recorded in TL.
- **Effect on selection**：Contributes +β in F1 and feeds Tnc in F2, encouraging novelty only subject to preservation and usefulness gates.

#### CR-ACOMP — A_compat

- **Mathematical definition, range and units**：Weighted success fraction of fixed integration/bridge queries connecting correctly to the existing system; dimensionless [0,1], not embedding similarity.
- **Calculation algorithm**：Check type, meaning, roundtrip and actual use with registered per-query oracles; apply success-K1 to pass 1, fail 0 and unknown [0,1]. Hard queries gate separately.
- **Applicability**：Requires pinned interfaces and checkable execution/formal success criteria.
- **Independent validation**：Use independent interface drivers, inverse bridges, type-correct but semantically wrong cases and slot-only cases.
- **Effect on selection**：Contributes +γ in F1 and multiplies Tnc. A=0 makes Tnc zero; other scores cannot compensate for failed hard connections.

#### CR-CMATCH — C_match

- **Mathematical definition, range and units**：C_match=1/(1+|K−K0|/κ_M), with positive κ_M in bits. Positive dimensionless value up to 1. This rational proxy completes an unspecified v1 algorithm; it is not Q.
- **Calculation algorithm**：Compute K/K0 by K6 and exact rational arithmetic. For intervals find minimum/maximum absolute differences before the monotone map; crossing K0 gives upper bound 1.
- **Applicability**：Requires the same encoding, bits, baseline and pinned κ_M. Unknown K prevents an exact point score.
- **Independent validation**：Independent fractions: zero difference gives 1, difference κ gives 1/2, equal positive/negative changes are symmetric, crossing intervals reach 1.
- **Effect on selection**：Contributes +δ in F1 as an explicitly added symmetry convention. Prefer simplification separately through JointPareto cost/gain rather than rewriting the original formula.

#### CR-NKNOWN — N_known

- **Mathematical definition, range and units**：Maximum construction-feature similarity max_y(1−K3(Fz,Fy)) in a fixed nonempty archive; dimensionless [0,1], not universal human knownness.
- **Calculation algorithm**：Pin corpus, universe and canonicalizer; compute all similarities and take the maximum. Confirmed empty archives are Undefined/EmptyCorpus; unread archives are Unknown and unsupported extraction is Unsupported. Small values require actual archive comparison.
- **Applicability**：Requires a comparable construction archive. Frequency, fame or model confidence do not substitute.
- **Independent validation**：Independent archive comparison, exact-copy 1, ID invariance, empty-archive Undefined and partial-search lower/upper bounds.
- **Effect on selection**：Contributes −η in F1. Corpus revisions require remeasurement; this is not a gate banning known useful candidates.

#### CR-TNC — T_nc

- **Mathematical definition, range and units**：Original sqrt(D_par D_sem) A_compat, dimensionless [0,1]. Either distance zero or A zero gives exact zero.
- **Calculation algorithm**：Reference shared metric nodes; use K7 square-root bounds and K4 multiplication. Non-square f64 results are not Exact. Missing inputs follow NoScalar/bounding rules.
- **Applicability**：All three components must be defined within one comparison scope. Scheduler geometry cannot replace these distances.
- **Independent validation**：Independent high precision and squared-bound checks. Dpar=1/4, Dsem=1, A=1/2 yields 1/4. Test zeros, monotonicity and undefined inputs.
- **Effect on selection**：Contributes +w1 in F2, requiring novelty and compatibility together, without replacing usefulness or hard duties.

#### CR-COVERAGE — Δcoverage

- **Mathematical definition, range and units**：Paired K2 improvement in the confirmed fraction of fixed coverage-validation queries explained/solved, dimensionless [-1,1].
- **Calculation algorithm**：Attach derivation/model/execution oracles per query; obtain baseline/candidate pass 1, fail 0 or unknown intervals under equal budgets; apply K2.
- **Applicability**：Fixed explanation/solution targets. Adding/removing queries creates a new scope. Keep final research holdouts separate.
- **Independent validation**：Independent finite-query enumeration and manual arithmetic: [1,0] to [1,1] gives 1/2. Reject exclusion of hard cases.
- **Effect on selection**：Contributes +a1 within G_struct. More claims alone do not increase coverage; actual oracle passes must increase.

#### CR-COMPRESSION — Δcompression

- **Mathematical definition, range and units**：(K0−Kz)/max(K0,Kz), dimensionless [-1,1], with K in bits. Valid compression gain requires checked required equivalence/preservation.
- **Calculation algorithm**：Encode artifacts plus necessary decoders/dictionaries under the same K6 rules; compare total lengths. External lookup hiding information is not free. Unchecked preservation is PrerequisitePending/NoScalar; failed preservation is Undefined/PrerequisiteFailed and candidate Ineligible. Unverified shortening is not valid gain.
- **Applicability**：Self-contained representations of common required information. Dropping source meaning is outside valid gain applicability.
- **Independent validation**：Independent encoder, hand-counted reversible examples, dictionary-inclusive lengths, required-information deletion and zero-denominator rejection.
- **Effect on selection**：Contributes +a2 to G_struct after preservation gates. Its tension with symmetric Q is an explicit original preference, not grounds for silently changing either term.

#### CR-BRIDGE — Δbridge

- **Mathematical definition, range and units**：K2 improvement in fixed cross-domain queries usable through checked bridges; dimensionless [-1,1], not edge count.
- **Calculation algorithm**：Register source/target/root-goal correspondence plus type/semantic/use oracles; compare baseline/candidate query success. Renamed duplicate edges share one primitive.
- **Applicability**：Checkable cross-logic/domain correspondence. Unmapped distinct tasks are Incomparable.
- **Independent validation**：Independent bridge drivers, finite models, added-but-useless edges and duplicate-bridge invariance.
- **Effect on selection**：Contributes +a3 to G_struct for usable new connections, not authority or universal semantic truth.

#### CR-PREDICT — Δpredict

- **Mathematical definition, range and units**：Brier(z0)−Brier(z), where Brier=Σwi(pi−yi)^2/W, pi∈[0,1], yi∈{0,1}. Dimensionless [-1,1]; multiclass requires another revision.
- **Calculation algorithm**：Pin prediction-validation data, observed labels, timestamps and model revisions. Freeze predictions before labels are acquired; aggregate paired squared-error differences by K2.
- **Applicability**：Binary tasks with authentic timed predictions/labels. Separate optimization validation from final independent holdouts. Missing labels remain Unknown.
- **Independent validation**：Independent rational Brier computation, perfect/fully wrong predictions, leakage checks and rejection of retrospectively altered predictions.
- **Effect on selection**：Contributes +a4 to G_struct as validation improvement; never reuse the final holdout while claiming independent performance.

#### CR-REDUNDANCY — Redundancy

- **Mathematical definition, range and units**：Σ_class max(0,n_class−1)/Σ_class n_class over role-specific semantic-unit multisets; dimensionless [0,1]. Required repetitions occupy distinct role classes.
- **Calculation algorithm**：Form classes only with checked equivalence, then count. A confirmed empty valid representation gives 0; incomplete extraction is Unknown. Surface word identity is insufficient.
- **Applicability**：Requires checkable roles/equivalence. Do not automatically remove safety repetitions in distinct roles.
- **Independent validation**：Independent class enumeration, synonymous duplicates, same words in distinct roles, unproved equivalence and empty-versus-unextracted cases.
- **Effect on selection**：Contributes −a5 to G_struct. Identity need not have zero redundancy: baseline Gstruct=−a5Redundancy0. Deletion remains separately authorized.

#### CR-GSTRUCT — G_struct

- **Mathematical definition, range and units**：Original subformula a1Δcoverage+a2Δcompression+a3Δbridge+a4Δpredict−a5Redundancy. Dimensionless range [−Σ1..5 ai, Σ1..4 ai].
- **Calculation algorithm**：Pin five child nodes and weights; evaluate by K4 without clipping signed gains. Expose shared evidence-dependency groups.
- **Applicability**：Requires all five valid values/bounds in one scope; strict formula never turns N/A into zero.
- **Independent validation**：Independent linear arithmetic, negative gains/penalty signs, nonzero baselines and endpoint range checks.
- **Effect on selection**：Contributes +w2 to F2, with child sensitivity w2ai. Retain components and never count correlated evidence as independent votes.

#### CR-PRESERVE — Preserve

- **Mathematical definition, range and units**：K1 weighted fraction of fixed source structural properties preserved, dimensionless [0,1]. Absolute, not a delta; identity usually gives 1.
- **Calculation algorithm**：Check source/target correspondence per property; aggregate pass/fail/unknown by success-K1. Gate the hard subset separately.
- **Applicability**：Fixed properties/equivalence for structural transformations. Syntactic isomorphism alone is not semantic preservation.
- **Independent validation**：Independent property checker, identity 1, one broken relation, relabeling invariance and missing traces.
- **Effect on selection**：Contributes +b1 to R_gain. Identity raw Rgain=b1 is allowed; use separately computed delta_S for baseline improvement.

#### CR-REBIND — Rebind

- **Mathematical definition, range and units**：Fraction of fixed source units genuinely rebound to changed roles/targets with useful map-back and passing use probes; dimensionless [0,1].
- **Calculation algorithm**：Compare bindings; exclude renaming; independently check type, semantics and root-goal use; apply success-K1. No change scores 0.
- **Applicability**：Reorganizations with checkable bindings/roles and common goals. Distinguish useful rebinding from broken references.
- **Independent validation**：Independent small-graph enumeration, rename-only zero, useful rebinding one and dangling-reference failure.
- **Effect on selection**：Contributes +b2 to R_gain. Expose overlap with Preserve/Reframe as original multi-aspect weighting; JointPareto deduplicates primitives.

#### CR-REFRAME — Reframe

- **Mathematical definition, range and units**：Weighted fraction of fixed perspective queries failing at baseline but explained after explicit reframing and checked map-back; dimensionless [0,1].
- **Calculation algorithm**：Confirm baseline failure, Delta frame change, candidate pass and root correspondence; apply K1 to I(base_fail AND mapped_pass). Unknown baseline stays unknown.
- **Applicability**：Requires a frame change mapping back to the root question. Merely making the task easier is ineligible.
- **Independent validation**：Independent query oracle, zero without reframing, valid perspective changes and fake gains from changing root goals.
- **Effect on selection**：Contributes +b3 to R_gain, requiring map-back before claiming satisfaction of original conditions.

#### CR-BREAK — BreakCost

- **Mathematical definition, range and units**：K1 failure fraction of fixed optional legacy compatibility/properties, dimensionless [0,1]. Hard duties and authority remain separate gates.
- **Calculation algorithm**：Run a fixed legacy suite under equal conditions; aggregate violation 1, preservation 0 and unknown intervals. Measure already-broken baseline conditions separately rather than silently exempting them.
- **Applicability**：Only scopes permitting soft-compatibility trade-offs; candidate-specific test/weight changes are forbidden.
- **Independent validation**：Independent compatibility driver, manual one-break arithmetic and rejection of relabeled hard duties.
- **Effect on selection**：Contributes −b4 to R_gain as a soft preference, never a price for permissions or mandatory safety.

#### CR-RGAIN — R_gain

- **Mathematical definition, range and units**：Original b1Preserve+b2Rebind+b3Reframe−b4BreakCost, dimensionless range [−b4,b1+b2+b3].
- **Calculation algorithm**：Read four same-scope child nodes and apply K4. Preserve is absolute; the word gain does not authorize automatic recentering.
- **Applicability**：Requires valid applicability and comparison scope for every child.
- **Independent validation**：Independent linear arithmetic, identity R=b1 with other children zero, penalty signs and shared evidence checks.
- **Effect on selection**：Contributes +w3 to F2. Nonzero raw identity is intentional; improvement is separately S(z)−S(z0).

#### CR-EXPLAIN — Δexplain

- **Mathematical definition, range and units**：Paired K2 gain in correct derivation/explanation scores on fixed explanation-validation queries, dimensionless [-1,1].
- **Calculation algorithm**：Register a checkable derivation oracle per query for pass 1/fail 0. LLM prose judgments are Estimated with pinned rubric/model/judge, never Exact formal guarantees.
- **Applicability**：Tasks with explanation or derivation oracles, limited to fixed queries rather than general explanatory ability.
- **Independent validation**：Independent proof/derivation checker; correct conclusions with invalid derivations, missing scope and merely longer prose.
- **Effect on selection**：Contributes +c1 to U_gain. Length or confidence alone gives no gain; unknown judgments require validation.

#### CR-DESIGN — Δdesign

- **Mathematical definition, range and units**：K2 improvement on a fixed design-acceptance suite, dimensionless [-1,1]. Per-probe scores are binary or a quantity with fixed m<M normalization bounds and a separate AcceptedInputRange: large-is-better uses clip((x−m)/(M−m),0,1); small-is-better uses 1−clip((x−m)/(M−m),0,1). m/M are saturation bounds, not the accepted input domain.
- **Calculation algorithm**：Validate AcceptedInputRange first. Values within that domain but outside normalization bounds m/M are explicitly saturated and retain raw value plus lower/upper clip flag; values outside AcceptedInputRange are InvalidInput. Pin direction in scope. Check units, measure both candidates with equal resources/environment and take paired differences. Unexecuted proposals are predicted.
- **Applicability**：Design tasks with execution, simulation or formal oracles. Bounds/targets cannot change per candidate. Hard acceptance gates separately.
- **Independent validation**：Independent acceptance drivers, m/M boundaries, unit conversion, rejection outside AcceptedInputRange, recorded saturation inside the accepted domain, and asymmetric-budget adversarial cases.
- **Effect on selection**：Contributes +c2 to U_gain. Preserve raw quantities/units alongside normalized effectiveness scores.

#### CR-TRANSFER — Δtransfer

- **Mathematical definition, range and units**：K2 gain on a pre-separated transfer-validation family, dimensionless [-1,1]. Scores use binary checks or CR-DESIGN's fixed normalization.
- **Calculation algorithm**：Check split digests against training/search families; execute baseline/candidate under equal transfer budgets; compute paired differences. Never feed the final research holdout into selection.
- **Applicability**：Requires explicit root-goal transfer mappings and leak-free family splits. No unsupported cross-task generalization.
- **Independent validation**：Independent split checks, duplicate-case detection, separate drivers, zero-/few-shot budget mismatches and holdout-leakage negatives.
- **Effect on selection**：Contributes +c3 to U_gain for observed transfer-validation selection, not universal transfer or independent research evaluation.

#### CR-SOLVE — Δsolve

- **Mathematical definition, range and units**：K2 improvement in correctness, feasibility or objective-quality scores on fixed root-goal solution queries, dimensionless [-1,1]. Pin binary or CR-DESIGN normalization per probe.
- **Calculation algorithm**：Give actual models/derivations/program outputs to independent oracles; check hard feasibility first; compare the same goal, queries and budgets. Format compliance alone is not solving.
- **Applicability**：Requires registered correctness/feasibility/quality oracles and explicit bridges between original and reconstructed contexts.
- **Independent validation**：Exhaustive small-domain optimality oracle, well-formatted wrong answers, easier solutions from dropped conditions and incomplete solvers.
- **Effect on selection**：Contributes +c4 to U_gain. Report runtime/tokens separately in their original resource units rather than silently mixing them.

#### CR-UGAIN — U_gain

- **Mathematical definition, range and units**：Original c1Δexplain+c2Δdesign+c3Δtransfer+c4Δsolve, dimensionless in [−Σci,Σci].
- **Calculation algorithm**：Evaluate four child nodes by K4, retaining dataset roles and evidence bases. Do not share its type with REC-U.
- **Applicability**：Defined scores against the same baseline/root goal. Inapplicable children yield NoScalar in strict F2 until an explicit different profile is selected.
- **Independent validation**：Independent linear arithmetic, negative gains, endpoints, overlapping query/evidence dependencies and N/A-masking negatives.
- **Effect on selection**：Contributes +w4 to F2. Predicted/Estimated inputs remain proposals of that strength, never promoted to observed gain.

#### CR-K — K, K0

- **Mathematical definition, range and units**：K(z)=8 len(E(z)) bits with positive integer K0=K(z0). This practical description-length proxy is not minimal program length or Kolmogorov complexity.
- **Calculation algorithm**：Encode semantic terms and required decoders/dictionaries as one self-contained Bundle term under K6. Pin shared runtime in ProfileDigest and treat both sides equally. Reference IDs cannot hide information externally.
- **Applicability**：Representations lowerable to finite acyclic typed terms, not complete canonical semantics of arbitrary Rust or prose.
- **Independent validation**：Independent byte encoder/decoder, golden hex, shortest-ULEB checks, alpha-renaming invariance, ordered-child changes and omitted dictionaries.
- **Effect on selection**：Shared node for C_match, Q and Δcompression. Dropping required meaning to shorten K fails gates; K does not judge truth.

#### CR-QPARITY — Q_parity

- **Mathematical definition, range and units**：Original exp(−|K−K0|/σ), positive σ in bits. Dimensionless (0,1] for finite K; equal simplification/complexification changes are symmetric.
- **Calculation algorithm**：Use shared K6 lengths, pinned σ and K7 to compute dmin/dmax. Lower=lowerExp(dmax/σ). If the K interval contains K0, an outer upper of 1 is allowed (Exact 1 requires evidence that K is exactly K0). If it excludes K0 and dmin>0, use upper=min(upperExp(dmin/σ),1/(1+dmin/σ)). Never label underflow/rounding to 0/1 as a mathematical Exact value.
- **Applicability**：Same encoding, unit, baseline and scale. Asymmetric alternatives require a different FormulaRevision and name.
- **Independent validation**：Independent high-precision exp: zero difference 1, difference σ e^-1, symmetry, K0-crossing and tiny-positive bounds.
- **Effect on selection**：Contributes +w5 to F2 as a symmetry preference. Display compression gain jointly without silently revising the original.

#### CR-RCOUNTER — R_counter

- **Mathematical definition, range and units**：K1 confirmed-failure fraction on a fixed nonempty adversarial suite; dimensionless [0,1], neither number of discovered counterexamples nor universal risk probability.
- **Calculation algorithm**：Run fixed cases under equal resources and independently recheck countermodels. Aggregate fail 1, pass 0 and unobserved [0,1]. Zero trials/budget means Unknown. Completed zero-failure suites give zero only for that finite suite.
- **Applicability**：Requires declared attack scope and quantification. A single counterexample refuting a hard universal claim gates separately.
- **Independent validation**：Independent countermodel evaluator, hidden hard cases, timeouts, duplicated counterexamples and single-counterexample universal refutation.
- **Effect on selection**：Contributes −w7 to F2. High values prompt repair/rechecking; low values do not establish safety or domain-wide correctness.

#### CR-BMEM — B_mem

- **Mathematical definition, range and units**：Normalize minimum bridge cost C in a closed finite registry of admissible nonsecurity conversions as C/(C+κ_B), with positive same-unit κ_B, yielding [0,1]. Map cost intervals through this monotone function endpointwise. Only proven path absence in the closed graph gives 1 plus Disconnected.
- **Calculation algorithm**：Enumerate and pin a closed finite admissible nonsecurity conversion graph, endpoints and nonnegative edge costs. For interval costs, run Dijkstra once on all lower edge weights and once on all upper edge weights to obtain the outer bound `C=[minPath(lower),minPath(upper)]`. Prove disconnection only when the edge registry is closed. Apply the monotone κ_B map to both endpoints. Break equal-cost path ties by canonical transition-payload sequence, never opaque IDs; merge identical canonical content/evidence paths into one cost witness; independently check witnesses. Timeouts retain Unknown and valid bounds. Early search failure never proves disconnection.
- **Applicability**：Nonsecurity introduction effort with typed fixed cost units. An open or incompletely observed edge registry makes path absence Unknown/NeedsEvidence. Permissions, prohibitions and hard safety boundaries cannot be purchasable edges.
- **Independent validation**：Independent Bellman-Ford/exhaustive small paths, zero cost, no path, negative-edge rejection, proportional unit conversion and path checking.
- **Effect on selection**：Contributes −w8 to F2 as effort preference. Denied/unknown permission separately gates execution regardless of score.

#### CR-V1 — S_Creative v1

- **Mathematical definition, range and units**：Formula F1 unchanged: a dimensionless preference index in [−(λ+η),α+β+γ+δ], neither probability nor universal creativity.
- **Calculation algorithm**：Pin six components, six coefficients and revisions; evaluate by K4. Evaluate baseline separately and return delta S1. Identity with A=C=1, a fixed archive containing the baseline construction (N=1), and Dctx=Dsem=L=0 gives γ+δ−η.
- **Applicability**：All referenced components valid in one scope with fixed coefficients/formula revision; no implicit zero filling.
- **Independent validation**：Independent rational evaluation, sign/substitution mutants, the 27-coefficient ledger, identity, NoScalar and degenerate all-zero weights.
- **Effect on selection**：OriginalScore-v1 maximizes S1 after gates using conservative bound comparisons. Extra TL penalties require another policy with expanded coefficients. Distinguish this from advisory display in default JointPareto.

#### CR-V2 — S_Creative v2

- **Mathematical definition, range and units**：Formula F2 unchanged. With G in [glo,ghi], R in [rlo,rhi], U in [ulo,uhi], lower bound is w2glo+w3rlo+w4ulo−w6−w7−w8 and upper bound w1+w2ghi+w3rhi+w4uhi+w5. Dimensionless.
- **Calculation algorithm**：Evaluate eight terms and their children in one DAG. Compute raw baseline and delta S2 separately. Identity with zero redundancy, Preserve=1, Q=1 and others zero gives w3b1+w5; retain −w2a5Redundancy0 for nonzero baseline redundancy.
- **Applicability**：Finite-formal comparisons defining every strict F2 component. Reduced scores omitting components need distinct names/profiles and are not completed F2.
- **Independent validation**：Independent calculator, child sign/weight mutants, nonzero baselines, interval boundaries and selection-changing cut vectors.
- **Effect on selection**：OriginalScore-v2 maximizes S2 after gates. JointPareto separately compares loss/gain. Never unknowingly charge the TL already inside S2 again.

### 6. Selection contract — connect measurements through adoption

1. Check identity/schema, current authority/evidence and hard obligations first. Hard failure makes that candidate ineligible; missing observations yield NeedsEvidence/Hold. Retaining proposals is separate from permission.
2. Compare only within one ComparisonScope. Premise reconstruction remains a primary objective, but branches without root-goal map-back have separate frontiers. Never mix different-context solutions into universal optimality.
3. `JointPareto-v1` uses objectives from a fixed `RootGoalProfile` (maximize), each active TL axis (minimize), applicable primitive gains (maximize) and soft costs (minimize). Pin GoalMetricId, raw unit/type, direction, oracle revision, scope/baseline and score mapping. Scalar normalization requires fixed bounds and a monotone mapping; keep multiple goals as separate Pareto axes. Scalarizing multiple goals requires a separate revision and explicit utility profile. Hard feasibility cannot be compensated by utility. Without a GoalProfile return NoRecommendation (candidate/alternative descriptions are still permitted); never invent an implicit goal score. Gains are CR-COVERAGE/COMPRESSION/BRIDGE/PREDICT/PRESERVE/REBIND/REFRAME/EXPLAIN/DESIGN/TRANSFER/SOLVE; costs are CR-REDUNDANCY/BREAK/RCOUNTER/BMEM. Fix applicability at scope construction. RootGoalScore and CR-SOLVE may be distinct quantities (absolute target value versus baseline-relative improvement). If they use the same GoalObservation primitive, share a dependency group and do not count independent evidence votes. Retain both Pareto axes only under explicit DeclaredOverlap. Do not additionally sum S1/S2 or Gstruct/Rgain/Ugain.
4. A conservatively dominates B only if every maximized component has A.lower>=B.upper, every minimized component A.upper<=B.lower, and at least one inequality is strict. Overlapping intervals are unresolved, not totally ordered by means. Only equal Exact values mean EquivalentInScope, not semantic equivalence. Equal interval bounds mean SameBounds/IndistinguishableAtCurrentResolution, not equality of unknown actual values. Retain unknown-bound candidates for exploration rather than silently discarding them.
5. Return one recommendation and important alternatives when evaluation permits. For one scalar goal, default representative key is highest goal lower bound, then lowest L_trans upper bound, then lexicographic lower gains/upper costs in declared primitive order. Multiple goals require a RootGoalProfile-pinned representative order/scalarization; without it return NoRecommendation and the important candidates. Tie-display keys use full canonical content, unresolved obligations, policy and evidence-lineage bytes, excluding CandidateId, sequence and nonsemantic timestamps. Merge exact content/evidence duplicates into one display representative while retaining all aliases. For same content with different evidence, show the evidence group; never choose by ID order. A representative is selected only by declared policy, not proof of unknown true superiority. Show other-scope candidates as important alternatives.
6. Upper-limited loss gates pass only when upper<=τ, fail when lower>τ, otherwise NeedsEvidence. Minimum-gain gates pass when lower>=τ and fail when upper<τ. Mandatory preservation uses τ=0; soft thresholds are required profile parameters, never invented defaults.
7. Only explicitly selected OriginalScore-v1/v2 modes maximize original raw scores. A strict interval score certifies A as winner only if lower(A)>upper(B) for every other eligible B. Overlapping intervals remain an unresolved top set. Equal Exact values may use canonical display tie-breaking, not superiority claims. For J=aL−bS2 the effective TL coefficient is a+bw6 (a+bλ for v1). Expand and record it, declaring any intentional repetition of the same TL. Default Pareto does not use this J.
8. Geometry proposes priorities only. The reference discrete scheduler uses priority=(1+u)(1+n)/(1+c_cost). u is the maximum active scalar bound-width divided by its declared range-width (zero-width ranges contribute 0); n is selected by a scope-pinned SchedulerNoveltyPolicyId. `DistanceToFrontierUnion-v1` preserves the existing reference behavior: K3 between proposed-branch canonical structural features and the union of the same features across the validated frontier. `NearestVerifiedCandidate-v1` is a distinct alternative: min over each verified frontier candidate y of K3(proposal features, features(y)); it is not an alias. Keep the union policy as the reference default for continuity, but do not conclude it is superior from the mathematical counterexample alone. These policies can reverse rankings; pin the policy before comparison and evaluate diversity, goal coverage and reproducibility at equal budgets (G35/TLCX-39) before any future default change. c_cost=min(1,declared estimated query-slot cost/B). Pin feature universe and cost-estimator revision; require u,n,c_cost∈[0,1]. Undefined inputs or empty active sets use canonical round-robin. B>0 is a fixed oracle-query dispatch budget; one slot executes one query, including timeouts, under registered logical-step/wall/memory caps. Assign ceil(B/8) slots by the cursor-bearing SchedulerCanonicalKey round-robin defined below; dispatch remaining slots by priority lower bound and break exact ties by SchedulerCanonicalKey. Recompute after each query. This uncalibrated allocation does not guarantee observation of every branch. Use alternate n_geo only with a registered GeometryPolicy meeting the next section's requirements. Geometry/distillation cannot overwrite semantic metrics without proposal→discrete validation→remeasurement.
8a. If inputs/calculations are intervals, propagate a bounded priority via K4. This is a deterministic scheduler policy under finite budgets, not a probability or a claim that latent priority order or answer utility is superior.

8b. `SchedulerCanonicalKey` is derived from candidate canonical typed-content bytes (excluding opaque CandidateId/ProbeId/BranchId, serialization order and nonsemantic timestamps), unresolved obligation/probe payloads, Definition/Oracle/Policy revisions and canonical evidence-payload digests. Evidence keys include semantic fields (kind/result/scope/issuer/backend/policy/dependency) and exclude opaque receipt/observation IDs, list order and transport timestamps. Never rank by raw IDs or arrival order. Merge duplicate work into one schedule group only when canonical keys and evidence lineage match exactly, retaining aliases to all candidates. Assign one opaque `BudgetSeriesId` to a TaskRun/ComparisonScope/SelectionPolicy and reuse it during same-run state recovery; it partitions state but is never an ordering key. Persist `FairnessState` for that BudgetSeriesId, including last-served key, cursor and frontier canonical digest/revision history sorted by SchedulerCanonicalKey. When the frontier changes, continue after the last key if it remains; otherwise use its canonical successor, wrapping as needed; insert new keys canonically after the cursor. A scope/policy change starts a new series. After each reserved query, advance the cursor. Without a valid cursor, start the first epoch at the smallest canonical key and report cross-epoch fairness as Unanchored. This reduces finite-budget bias but does not guarantee processing every candidate. Test that ID relabeling alone cannot change priorities/recommendations.
9. Changed values need not always change winners (zero weights, failed gates or dominance may intervene). For every active component build fixed-other-condition, nonzero-sensitivity, sufficient-margin fixtures and observe measurement→frontier/order→recommendation or next-query changes. A cut yields NeedsEvidence or explicit NoRecommendation, not a cached winner.

### 6A. Poincare search distance and applicability limits

Search geometry proposes allocation priorities from observed state transitions; it is not a physical-space measure, TL truth value or causal law. `PoincareBallDistance-v1` uses dimensionless coordinates, curvature −κ_curv (κ_curv>0) and the ball `B={x∈R^d:κ_curv||x||_2^2<1}` with distance

`d_κ(x,y)=acosh(1+2κ_curv||x−y||_2^2/((1−κ_curv||x||_2^2)(1−κ_curv||y||_2^2)))/sqrt(κ_curv)`.

For κ_curv=1 this is the standard unit Poincare-ball distance; general κ follows by scaling x'=sqrt(κ_curv)x. It is nonnegative, symmetric, satisfies d_κ(x,y)=0 iff x=y, and obeys the triangle inequality. Bind κ_curv, novelty scale s_geo>0, μ and every coordinate in GeometryPolicyDigest. Accept only finite dimensionless rational coordinates or bounded intervals of the same dimension d>=1, with fixed boundary margin 0<μ<1. Interval coordinates are DomainValid only when the entire coordinate box satisfies `κ_curv||x||²<=1−μ`; unproven membership is NeedsEvidence, violation is InvalidInput. Rounded floats enter as their exact binary rational bit patterns or as error intervals, never as exact semantic truth of continuous coordinates. Never clip boundary, NaN, nonfinite or overflow to success.

K8 internal numeric contract: fix p_out=64 in GeometryPolicy. For each coordinate endpoint, use the following nonnegative rational `t` and identity:

`t=κ_curv*||x-y||_2^2/((1-κ_curv*||x||_2^2)(1-κ_curv*||y||_2^2))`

`acosh(1+2*t)=2*atanh(sqrt(t/(1+t)))`

Use K4 to compute a nonnegative rational outer interval t=[tL,tU] from coordinate inputs. Compute `||x-y||²` and the endpoint norm-squares with K4's dedicated `square` operation on each coordinate-difference/input interval, then sum; generic dependent interval products X×X are not allowed to create negative norm-square lower bounds. For each rational endpoint t_i, start at p_inner=80 and outward-bound `q_i=sqrt(t_i/(1+t_i))` with K7. Its endpoint distance is `dκ(t_i)=2*atanh(q_i)/sqrt(κ_curv)`. Enclose atanh(q) using `S_N(q)=Σ_(k=0)^N q^(2k+1)/(2k+1)`: lower S_N(qL), upper `S_N(qU)+qU^(2N+3)/((2N+3)(1−qU²))`. Increase N from 0 to 256. An endpoint computation converges when the numeric width (upper minus lower) of its distance enclosure, including q and sqrt(κ_curv) interval operations, is <=2^-p_out. Form the final outward interval from the distance lower bound at tL through the distance upper bound at tU. Increasing N alone cannot reduce q's sqrt-rounding width, so if numeric width is unmet, increase p_inner by 32, recompute q and sqrt(κ_curv), and cap at 256. Preserve semantic input uncertainty between tL/tU in the final output interval; do not confuse it with either endpoint's numeric width. If qU=1 or endpoint slack is unmet, increase p_inner and recompute. If the condition remains at the precision/resource cap, return ResourceExhausted with a valid outer bound when available, otherwise NoScalar; never fabricate Exact/infinite distance. Exact t=0 gives d=0.

This distance does not define how states obtain coordinates. EmbeddingPolicyId must pin the transition-graph snapshot/edge evidence, feature-to-node correspondence, embedding objective, initialization, optimizer/version/seed, dimension, curvature, novelty scale s_geo, stopping rule/budget and held-out relation checks. Coordinates without that registration are Unsupported; never infer them from IDs or an LLM proposal. An empty validated frontier uses round-robin. Otherwise bound `d_near=min_(y∈F)d_κ(x_proposal,y)` as [dL,dU] and use `n_geo=[1−upperExp(dL/s_geo),1−lowerExp(dU/s_geo)]` for search priority only, with s_geo>0 in the same dimensionless distance unit. This is an Estimated scheduling proposal, not calibrated semantic novelty. Propagate priority intervals with K4 and dispatch by lower priority bound; ties use SchedulerCanonicalKey. This is not a superiority claim. Do not infer directionality from a symmetric distance; use registered `DirectedTransitionCost` separately. The Poincare-ball model follows [Nickel & Kiela, NeurIPS 2017](https://proceedings.neurips.cc/paper/2017/file/59dfa2df42d9e3d41f5b02bfc32229dd-Paper.pdf). This contract fixes the metric calculation but leaves geometric search unconnected until an EmbeddingPolicy/production consumer is registered; fall back to the discrete scheduler.

### 7. Rust implementation sequence, wire and resource contracts

This section specifies future implementation, not source changes in this task.

| Owner | Pure kernels/responsibilities | Forbidden |
| --- | --- | --- |
| substrate | Digest32/typed IDs, checked binary, bounded rational codec, spans/budgets | JSON core truth, raw struct dumps, unsafe decode |
| core | ComparisonScopeId, ProbeId, MetricId, separate state enums | Scores converted to Authority |
| tl | K1–K7, four axes/records, TLReductionPolicy, checked formula-DAG input | Unknown-to-zero, heuristic-to-truth promotion |
| world | Pure S1/S2 evaluation, Pareto, K8/registered GeometryPolicy scheduling and alternatives | Geometry skipping validation |
| solvers | Logic-specific results and model/proof/countermodel payloads | SAT/entailment/optimality conflation |
| runtime | Same-snapshot producer→measurement→consumer→Coding/Media/output | Doctor success as production connection |

Implementation order: metric registry; scope validation; rational/intervals; K1/K2/K3; K6; K7; leaves; S1/S2; Pareto/gates. K8/Geometry is a separate world-slice dependency and connects later only when a registered coordinate producer and consumer exist; then production consumers, cuts/invalidation/resume. Implement leaves first; formula evaluators do not invent component measurements.

At each required-profile slice start, fix the required oracle registry, payload schemas and positive/negative inputs. Returning Unsupported alone does not complete mandatory implementation.

Give every MetricId a fixed enum discriminant and DefinitionRevision. Separate Rust types for MeasurementValue (ExactRational/BoundedRational/Estimated), EvaluationState, InputBasis, SemanticValidation and IndependenceStatus. Private constructors validate units/scopes/ranges; Deserialize cannot promote reports directly to checked values. Existing `lc631-substrate::reported::Measurement` is a report container, not the checked scientific-measurement type.

Each kernel returns `Result<MeasurementReport, MeasurementError>`. Exhaustively match variants; forbid `_ => 0`, `unwrap_or_default`, implicit f64 casts and saturating arithmetic that continues as success. Retain MissingInput/Unavailable/Unsupported/Conflict reasons. Private Interval constructors enforce finite rational endpoints with lo<=hi. Exact labels arithmetic precision only; observation and semantic-validation fields remain separate.

Use TLCX-03 checked binary frames for rational values, state tags and scope/policy/evidence references. JSON remains an additive external view. Rational encoding uses sign:u8, positive denominator, shortest magnitudes, GCD=1 and positive zero. Bound trace chunks to 1MiB and logical frames to 16MiB; remove no existing public fields. Persist no native Rust layouts or pointers.

Cache keys include every source/target/context/baseline/probe/corpus/oracle/logic/encoding/policy/evidence revision. Any change gives Stale and remeasurement; never restore Completed from old scores. Active leases, receipts, journals, models and benchmarks are not disposable caches. No cache deletion occurs in this specification task. Temporary metric caches have fixed quotas and owned leases; only recomputable objects are eviction candidates.

### 8. Independent goldens, adversarial tests and exit criteria

The table gives hand-calculable or independent-calculator specification vectors. Synthetic formula inputs do not establish that metrics can actually produce those values; keep them separate from end-to-end fixtures.

| Vector | Input and expected result |
| --- | --- |
| G01 | K1 weights (1,3), losses (0,1) →3/4 |
| G02 | Same weights, losses (0,Unknown[0,1]) →[0,3/4] |
| G03 | K3 {a,b}/{b,c}, unit weights →2/3 |
| G04 | Paired baseline (1,0), candidate (1,1) →1/2 |
| G05 | T: Dpar=1/4,Dsem=1,A=1/2 →1/4 |
| G06 | C_match: K0=80,κ=80,K=160 →1/2 |
| G07 | Q: K0=160,σ=80,K=80 and 240 →both exp(-1)≈0.36787944117144232160 |
| G08 | Q: K∈[80,240],K0=160,σ=80 →[exp(-1),1] |
| G09 | Uniform synthetic identity, A=C=1, archive includes exact baseline so N=1, Dctx=Dsem=L=0 →S1=1/6 |
| G10 | Uniform identity, Preserve=1,Q=1,Rebind=Reframe=BreakCost=Redundancy=0,T=G=U=L=Rcounter=Bmem=0 →S2=5/32 |
| G11 | G10 with Redundancy=1/4 →Gstruct=−1/20,S2=3/20 |
| G12 | Raw delta S is S(z) minus measured S(z0); identity delta is zero even if raw score is not |
| G13 | Loss [0.1,0.2],τ=0.2 passes; [0.2,0.3] needs evidence; [0.21,0.3] fails |
| G14 | c=3,κ_B=1 →Bmem=3/4; proven no path →1+Disconnected |
| G15 | Δcompression K0=80,K=40 with preservation pass →1/2; failed preservation →Ineligible |
| G16 | Brier baseline p=0,y=1, candidate p=1/2 →Δpredict=3/4 |
| G17 | K4 `[-2,3]×[-4,5]` →`[-12,15]`; divisor interval containing 0 →Undefined/DivisionByZero |
| G18 | Error e=5, epsilon=2, r=10 (same unit) →ell=3/10; canonicalized identity 1s→1000ms has zero error |
| G19 | UNSAT report without checked proof/enumeration →ValidationBasis=SolverReported, not independent validation |
| G20 | Q, K∈[200,240],K0=160,σ=80 →[exp(-1),exp(-1/2)] |
| G21 | Poincare κ=1, x=(0,0), y=(1/2,0) →d=acosh(5/3)=ln3≈1.098612288668109691 |
| G22 | No comparable registered RootGoalProfile →NoRecommendation; retain alternatives |
| G23 | Poincare κ=1 on one diameter: d(0,1/4)=ln(5/3), d(1/4,1/2)=ln(9/5), d(0,1/2)=ln3; the first two sum to the third. Node-ID relabeling leaves distances unchanged |
| G24 | Goal raw range [10,30], v=15 →max score 1/4, min score 3/4; v=31→InvalidInput |
| G25 | Two tied branches A<B by canonical key; relabel IDs leaves dispatch unchanged. Next budget epoch begins after persisted cursor even when frontier revision changed |
| G26 | CR-DESIGN accepted domain [-100,100], normalize [0,10], maximize x=12 →1 with SaturatedHigh; x=101→InvalidInput |
| G27 | B_mem: two paths with cost intervals [1,2]+[1,2] and [3,4] →minimum cost [2,4]; κ_B=2 gives B_mem=[1/2,2/3]. Open graph/no path observation →Unknown, not Disconnected |
| G28 | K4 family min{[0,10],[5,6]} enclosure →[0,6]; max →[5,10]; empty family min/max →Undefined/NoApplicable |
| G29 | Noncrossing Q with dmin/σ=2^-1000: outward exp bound may equal 1, but rational upper `1/(1+2^-1000)<1` retains strict noncrossing |
| G30 | Poincare κ=1, x=(0,0), y=(1/2,1/2), μ=1/4: q=sqrt(1/2); K8 refines internal precision above 64 bits until endpoint numerical slack<=2^-64 or reports ResourceExhausted |
| G31 | `d_near=[0,1]`, `s_geo=1`: `n_geo` encloses `[0,1−e^-1]` with ordered endpoints |
| G32 | Poincare κ=1, x=(1/2,0), y=(0,1/2): exact `t=8/9`, `q²=8/17`; K8 raises precision above 64 bits or reports bounded ResourceExhausted, never false convergence |
| G33 | Poincare κ=1, x∈[0,1/10], y=1/2: the sound enclosure includes `[ln(27/11),ln3]`, width `ln(11/9)`; input uncertainty is not numerical failure |
| G34 | `square([-1/10,1/10])` returns `[0,1/100]` and cannot create a negative norm-square lower bound |
| G35 | Frontier `{{a},{b}}`: proposal `{a}` vs `{a,b}` gives union-distance `1/2,0` and nearest-verified-candidate distance `0,1/2`; bind each to a different SchedulerNoveltyPolicyId |
| G36 | A new feature outside the fixed K3 universe remains InvalidInput/UniverseMismatch until a new ComparisonScope is established |
| G37 | Missing/unverified EmbeddingPolicy remains Unsupported; a search proposal cannot create authority or exact semantic novelty |

Assign positive, negative and selection/exploration-impact test IDs to all 40 contracts. Records test ledger→gate/query effects rather than scalar ranks. Fill at least 120 contract-test slots with per-item execution receipts. Never generate goldens by re-calling production functions. Combine independent exhaustive assignment oracles for n<=8, a separate K6 encoder, independent rational/high-precision implementations and hand arithmetic.

Also require original-variable/sign mutants, all 27 coefficients, duplicate TL penalties, unknown versus zero, N/A, empty denominators, invalid σ, numeric budgets, source-sidecar leakage, inconsistent Γ, ASP brave/cautious, Both/Neither, reversed time/clocks, pixels/world, common-oracle errors, holdout leakage, uncalibrated 2D→3D, shared evidence, candidate-mask tampering, unobserved hosts, stale resume values and GPU-error rank reversal.

Specification exit: original-formula/component mapping, five fields for 40 contracts, equivalent JA/EN, numerical examples and independent adversarial reading, explicitly retaining implementation incompleteness. Future implementation exit: all 120 slots plus boundary tests and observed leaf→formula→selection→production/cut/invalidation/resume connections; satisfy source/package/host gates separately. A written specification alone is not implementation completion.

### 8A. Fixed registry and test ledger

Fix MetricId discriminants to this table. P/N/S are mandatory positive, negative and selection-impact tests, using the corresponding catalogue validation/impact requirements. Slot presence is not execution success.

| MetricId | Tag (u16) | Positive | Negative | Selection / exploration |
| --- | --- | --- | --- | --- |
| TL-RECON | 0x0100 | TL-RECON-P | TL-RECON-N | TL-RECON-S |
| TL-SEM | 0x0101 | TL-SEM-P | TL-SEM-N | TL-SEM-S |
| TL-TIME | 0x0102 | TL-TIME-P | TL-TIME-N | TL-TIME-S |
| TL-SPACE | 0x0103 | TL-SPACE-P | TL-SPACE-N | TL-SPACE-S |
| TL-TOTAL | 0x0104 | TL-TOTAL-P | TL-TOTAL-N | TL-TOTAL-S |
| TL-PATH | 0x0105 | TL-PATH-P | TL-PATH-N | TL-PATH-S |
| TL-MODAL | 0x0106 | TL-MODAL-P | TL-MODAL-N | TL-MODAL-S |
| REC-D | 0x0107 | REC-D-P | REC-D-N | REC-D-S |
| REC-DELTA | 0x0108 | REC-DELTA-P | REC-DELTA-N | REC-DELTA-S |
| REC-U | 0x0109 | REC-U-P | REC-U-N | REC-U-S |
| REC-G | 0x010a | REC-G-P | REC-G-N | REC-G-S |
| CR-DCTX | 0x010b | CR-DCTX-P | CR-DCTX-N | CR-DCTX-S |
| CR-DPAR | 0x010c | CR-DPAR-P | CR-DPAR-N | CR-DPAR-S |
| CR-DSEM | 0x010d | CR-DSEM-P | CR-DSEM-N | CR-DSEM-S |
| CR-ACOMP | 0x010e | CR-ACOMP-P | CR-ACOMP-N | CR-ACOMP-S |
| CR-CMATCH | 0x010f | CR-CMATCH-P | CR-CMATCH-N | CR-CMATCH-S |
| CR-NKNOWN | 0x0110 | CR-NKNOWN-P | CR-NKNOWN-N | CR-NKNOWN-S |
| CR-TNC | 0x0111 | CR-TNC-P | CR-TNC-N | CR-TNC-S |
| CR-COVERAGE | 0x0112 | CR-COVERAGE-P | CR-COVERAGE-N | CR-COVERAGE-S |
| CR-COMPRESSION | 0x0113 | CR-COMPRESSION-P | CR-COMPRESSION-N | CR-COMPRESSION-S |
| CR-BRIDGE | 0x0114 | CR-BRIDGE-P | CR-BRIDGE-N | CR-BRIDGE-S |
| CR-PREDICT | 0x0115 | CR-PREDICT-P | CR-PREDICT-N | CR-PREDICT-S |
| CR-REDUNDANCY | 0x0116 | CR-REDUNDANCY-P | CR-REDUNDANCY-N | CR-REDUNDANCY-S |
| CR-GSTRUCT | 0x0117 | CR-GSTRUCT-P | CR-GSTRUCT-N | CR-GSTRUCT-S |
| CR-PRESERVE | 0x0118 | CR-PRESERVE-P | CR-PRESERVE-N | CR-PRESERVE-S |
| CR-REBIND | 0x0119 | CR-REBIND-P | CR-REBIND-N | CR-REBIND-S |
| CR-REFRAME | 0x011a | CR-REFRAME-P | CR-REFRAME-N | CR-REFRAME-S |
| CR-BREAK | 0x011b | CR-BREAK-P | CR-BREAK-N | CR-BREAK-S |
| CR-RGAIN | 0x011c | CR-RGAIN-P | CR-RGAIN-N | CR-RGAIN-S |
| CR-EXPLAIN | 0x011d | CR-EXPLAIN-P | CR-EXPLAIN-N | CR-EXPLAIN-S |
| CR-DESIGN | 0x011e | CR-DESIGN-P | CR-DESIGN-N | CR-DESIGN-S |
| CR-TRANSFER | 0x011f | CR-TRANSFER-P | CR-TRANSFER-N | CR-TRANSFER-S |
| CR-SOLVE | 0x0120 | CR-SOLVE-P | CR-SOLVE-N | CR-SOLVE-S |
| CR-UGAIN | 0x0121 | CR-UGAIN-P | CR-UGAIN-N | CR-UGAIN-S |
| CR-K | 0x0122 | CR-K-P | CR-K-N | CR-K-S |
| CR-QPARITY | 0x0123 | CR-QPARITY-P | CR-QPARITY-N | CR-QPARITY-S |
| CR-RCOUNTER | 0x0124 | CR-RCOUNTER-P | CR-RCOUNTER-N | CR-RCOUNTER-S |
| CR-BMEM | 0x0125 | CR-BMEM-P | CR-BMEM-N | CR-BMEM-S |
| CR-V1 | 0x0126 | CR-V1-P | CR-V1-N | CR-V1-S |
| CR-V2 | 0x0127 | CR-V2-P | CR-V2-N | CR-V2-S |

Additional fields: DefinitionRevision, ComparisonScopeId, MeasurementBindingDigest, GoalProfileId/GoalMetricId/RepresentativePolicyId, Source/Target/Context/BaselineRevision, ProbeSetDigest, LogicProfile, OutcomeKind, ComparatorId, TransformId, AcceptedInputRange, NormalizationMap, EncodingRevision, PolicyDigest, InputBasis, NumericPrecision, SemanticValidation, SemanticResult, ValidationBasis, OracleRevision, IndependenceStatus, SchedulerNoveltyPolicyId, SchedulerCanonicalKey/FairnessStateRevision/BudgetSeriesId/FairnessCursor, EvidenceRefs, DatasetRole, GeometryPolicyId/EmbeddingPolicyId, ResourceStatus and DependencyMetricIds. Neither timestamps nor source hashes alone establish independence/authenticity.

### 9. Adversarial review record, residual uncertainty and stop/resume

The 2026-10-29 hostile re-audit corrected definition mismatches without changing either original formula:

- Aligned the source→target temporal map with its error formula, separating canonical-unit conversion from allowed discrepancy epsilon and positive saturation scale r.
- Bound vector/spatial comparators, tolerances, NormIds and payloads to scope-fixed Probes/MeasurementBindings.
- Defined general interval multiplication/division/absolute value/exp endpoints, formula-DAG value identity and zero-division state.
- Separated solver logical outcomes from validation basis; UNSAT reports or solver agreement alone do not count as independent validation.
- Fixed RootGoalProfile/GoalMetric, scalarization and multi-objective representative rules; no implicit recommendation without a goal oracle.
- Separated unresolved overlapping OriginalScore intervals from Pareto representative selection.
- Added Poincare-ball domain, curvature and numerical calculation, while explicitly keeping coordinate generation, embedding objective and an unregistered production consumer Unsupported/fallback.
- Defined standard monotone goal-range maps, explicit multi-goal policy, SchedulerCanonicalKey and a fairness cursor that survives frontier revisions, closing ID-relabeling effects on scheduling order.
- Separated metric saturation bounds from raw AcceptedInputRange for components such as CR-DESIGN, avoiding conflict with K7's no-generic-clipping rule.
- Specified B_mem as an exact/interval shortest-path problem over a closed finite graph; absence in an open registry cannot be promoted to Disconnected.

Separate rational/high-precision decimal calculations checked representative vectors for interval arithmetic, epsilon normalization, goal maps, CR-DESIGN saturation, S2 baseline, one-sided Q intervals, B_mem interval mapping and the Poincare/atanh enclosure. These are independent recalculations of selected numbers, not formal proofs, implementation of all components, or calibration of general semantic validity.

The remaining hard design blocker is the EmbeddingPolicy that generates Poincare coordinates and its production-scheduler consumer. Fixing the distance equation alone does not mean non-Euclidean search is running or activated.

Finite probes, semantic validity of feature extraction, oracle independence, weights, generalization and free-language/creative-work evaluation still require calibration and research evaluation. Exact finite-fragment arithmetic is not universal semantic distance. The intuitive mathematics/logic, programs, natural language, creative works and other actions classification informs task design but is not five direct numeric axes.

### 9A. Reconciliation of the attached v1.1 mathematical re-audit / finite-formal-v1.4 addendum

The reviewed source was `epistesys_tlcx_v1_1_mathematical_reaudit_2026-10-29.md` (SHA-256 `c4a6021dfd59303bec73f45f3d50712c33a603781e43f08526605e1c5d76f28b`). It targets `finite-formal-v1.1`; therefore its findings F-01–03 are checked against the current v1.3 text rather than treated as current defects by default.

- **F-01**: Current v1.3 already uses the monotonic endpoint expression for `n_geo`: `[1−upperExp(dL/s_geo),1−lowerExp(dU/s_geo)]`. No formula correction is needed; v1.4 adds the G31 endpoint-order regression vector.
- **F-02/F-03**: Current v1.3 starts at `p_inner=80`, adaptively increases it in 32-bit steps, and separates endpoint numerical width from input uncertainty across `tL..tU`. This differs from v1.1's fixed same-precision and total-distance-width stopping rule. v1.4 adds the irrational-q G32 and interval-coordinate G33 regression vectors to prevent false convergence.
- **D-02**: Generic interval multiplication of the same uncertain value may create an unnecessarily negative lower bound for a square. v1.4 adds the dedicated K4 `square([a,b])` rule and requires K8 norm-squares to sum coordinate-wise squares; G34 tests the nonnegative enclosure.
- **D-01**: K3 distance to the frontier union is defined but serves a different purpose from nearest-candidate distance and can reverse rankings. v1.4 names these separately as `DistanceToFrontierUnion-v1` and `NearestVerifiedCandidate-v1` SchedulerNoveltyPolicyIds. Preserve the current union reference behavior; do not replace it with nearest distance without evidence. Compare under identical scope and budgets (G35) before any default change.
- **B-01/B-02**: Features outside the fixed K3 universe still yield `UniverseMismatch` and require a new ComparisonScope/revision. The unconnected EmbeddingPolicy/production consumer remains Unsupported/fallback, never success (G36/G37).

`finite-formal-v1.4` denotes this specification addendum only. It does not change or validate the product version, original F1/F2 formulas, implementation, calibration, performance, or semantic effectiveness.

Assurance-Compiler remains auxiliary for selected TLCX Rust units, retaining needs_evidence, blocked_with_diagnostics and unreviewed scope. It does not certify this Markdown, mathematical proof, calibration or repository-wide safety. RepoSeiri assists document/reference consistency, not mathematical truth.

After fixing this specification, send one completion notice through Kusuri and preserve the delivery result in a separate private receipt. Then stop every task in this session. On user-directed resume, consult the [progress/handoff](tlcx-implementation-progress-2026-10-28.md), preserving changes and rechecking TLCX-01's pending exit, TLCX-02 drafts and unobserved host G13. This task performs no automatic resume, commit/push, version or plugin update.

